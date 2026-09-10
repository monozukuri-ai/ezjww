from __future__ import annotations

import math
from functools import lru_cache
from pathlib import Path
from typing import Any, Iterable


def _load_matplotlib() -> tuple[Any, Any]:
    try:
        import matplotlib.pyplot as plt
        from matplotlib import patches
    except Exception as exc:  # pragma: no cover - runtime dependency path
        raise ImportError(
            "matplotlib is required for plotting. Install with: pip install 'ezjww[plot]'"
        ) from exc
    plt.rcParams["pdf.fonttype"] = 42
    plt.rcParams["ps.fonttype"] = 42
    return plt, patches


def _normalize_layer_filter(layers: Iterable[str] | None) -> set[str] | None:
    if layers is None:
        return None
    normalized = {layer.strip() for layer in layers if layer.strip()}
    return normalized or None


def _aci_to_color(aci: int) -> Any:
    mapping = {
        1: "#ff0000",
        2: "#ffff00",
        3: "#00ff00",
        4: "#00ffff",
        5: "#0000ff",
        6: "#ff00ff",
        7: "#000000",
        8: "#808080",
        9: "#c0c0c0",
    }
    if aci in mapping:
        return mapping[aci]

    # Keep this generated AutoCAD Color Index palette in step with
    # `crates/ezjww-core/src/dxf.rs::aci_rgb`.
    if 10 <= aci <= 249:
        levels = (1.0, 0.65, 0.5, 0.3, 0.15)
        group, shade = divmod(aci - 10, 10)
        high = levels[shade // 2]
        low = high / 2.0 if shade % 2 else 0.0
        step = (group % 4) / 4.0
        rise = low + (high - low) * step
        fall = high - (high - low) * step
        sector = group // 4
        if sector == 0:
            red, green, blue = high, rise, low
        elif sector == 1:
            red, green, blue = fall, high, low
        elif sector == 2:
            red, green, blue = low, high, rise
        elif sector == 3:
            red, green, blue = low, fall, high
        elif sector == 4:
            red, green, blue = rise, low, high
        else:
            red, green, blue = high, low, fall

        # Rust's positive `f64::round` rounds a half away from zero.
        def quantize(value: float) -> int:
            return int(value * 255.0 + 0.5)

        return "#{:02x}{:02x}{:02x}".format(
            quantize(red),
            quantize(green),
            quantize(blue),
        )

    gray = (0x54, 0x76, 0x98, 0xBB, 0xDD, 0xFF)
    if 250 <= aci <= 255:
        value = gray[aci - 250]
        return f"#{value:02x}{value:02x}{value:02x}"

    return "#000000"


def _entity_color(aci: int, *, monochrome: bool) -> Any:
    if monochrome:
        return "#000000"
    return _aci_to_color(aci)


# JWC rendering-policy dash patterns in output millimeters (on, off, ...).
_JWC_PATTERNS_MM: dict[str, tuple[float, ...]] = {
    "JWC_DASHED1": (1.25, 1.25),
    "JWC_DASHED2": (2.5, 2.5),
    "JWC_DASHED3": (0.6, 0.6),
    "JWC_DASHDOT1": (7.5, 1.25, 1.25, 1.25),
    "JWC_DASHDOT2": (12.5, 2.5, 2.5, 2.5),
    "JWC_DIVIDE1": (7.5, 1.25, 1.25, 1.25, 1.25, 1.25),
    "JWC_DIVIDE2": (12.5, 2.5, 2.5, 2.5, 2.5, 2.5),
}


def _line_style(line_type: str) -> Any:
    name = line_type.upper()
    if name == "CONTINUOUS":
        return "-"
    if name in _JWC_PATTERNS_MM:
        # Preview approximation in points; the exact millimeter pattern is applied
        # after the axes scale is known (see ``patterned_artists``).
        return (0.0, tuple(value * 5.6 for value in _JWC_PATTERNS_MM[name]))
    if name == "DASHED":
        return (0.0, (7.0, 3.0))
    if name in {"DASHED2", "DASHEDX2"}:
        return (0.0, (14.0, 5.0))
    if name == "DASHDOT":
        return (0.0, (10.0, 3.0, 0.0, 3.0))
    if name in {"DASHDOT2", "DASHDOTX2"}:
        return (0.0, (18.0, 4.0, 0.0, 4.0))
    if name == "CENTER":
        return (0.0, (18.0, 4.0, 4.0, 4.0))
    if name in {"CENTER2", "CENTERX2"}:
        return (0.0, (28.0, 6.0, 6.0, 6.0))
    if name == "DOT":
        return (0.0, (0.0, 3.0))
    if name in {"DOT2", "DOTX2"}:
        return (0.0, (0.0, 5.0))
    return "-"


def _line_capstyle(line_type: str) -> str | None:
    name = line_type.upper()
    if name in {"DASHDOT", "DASHDOT2", "DASHDOTX2", "DOT", "DOT2", "DOTX2"}:
        return "round"
    return None


def _apply_line_capstyle(artist: Any, capstyle: str | None) -> None:
    if capstyle is None:
        return
    if hasattr(artist, "set_dash_capstyle"):
        artist.set_dash_capstyle(capstyle)
    elif hasattr(artist, "set_capstyle"):
        artist.set_capstyle(capstyle)


def _entity_linewidth(entity: dict[str, Any], default: float) -> float:
    try:
        line_weight = int(entity.get("line_weight", -3))
    except (TypeError, ValueError):
        return float(default)
    if line_weight <= 0:
        return float(default)
    return max(0.05, line_weight * 72.0 / 2540.0)


def _filled_polygon_edge_kwargs(
    color: Any,
    line_width: float,
    *,
    draw_edges: bool,
) -> dict[str, Any]:
    if draw_edges:
        return {
            "edgecolor": color,
            "linewidth": max(0.05, line_width * 0.8),
            "antialiased": True,
        }
    return {
        "edgecolor": "none",
        "linewidth": 0.0,
        "antialiased": False,
    }


@lru_cache(maxsize=1)
def _text_font_properties() -> Any | None:
    try:
        from matplotlib import font_manager
    except Exception:  # pragma: no cover - runtime dependency path
        return None

    candidates = (
        "TakaoPGothic",
        "TakaoGothic",
        "VL PGothic",
        "VL Gothic",
        "Noto Sans CJK JP",
        "IPAPGothic",
        "Yu Gothic",
        "Hiragino Sans",
    )
    for family in candidates:
        try:
            path = font_manager.findfont(family, fallback_to_default=False)
        except Exception:
            continue
        if path:
            return font_manager.FontProperties(fname=path)
    return None


def _data_unit_to_points(ax: Any) -> float:
    fig = ax.figure
    fig.canvas.draw()

    x0, x1 = ax.get_xlim()
    y0, y1 = ax.get_ylim()
    data_width = abs(float(x1) - float(x0))
    data_height = abs(float(y1) - float(y0))

    bbox = ax.get_window_extent()
    width_points = float(bbox.width) * 72.0 / float(fig.dpi)
    height_points = float(bbox.height) * 72.0 / float(fig.dpi)

    scales = []
    if data_width > 0.0 and width_points > 0.0:
        scales.append(width_points / data_width)
    if data_height > 0.0 and height_points > 0.0:
        scales.append(height_points / data_height)

    return min(scales) if scales else 1.0


def _text_em_height(entity: dict[str, Any], text_em_scale: float) -> float:
    return _as_float(entity.get("height"), 2.5) * _as_float(text_em_scale, 1.0)


def _as_float(value: Any, fallback: float) -> float:
    try:
        result = float(value)
    except (TypeError, ValueError):
        return fallback
    return result if math.isfinite(result) else fallback


def _text_fontsize(height: Any, text_scale: float, unit_to_points: float) -> float:
    size = _as_float(height, 2.5)
    return max(0.1, size * float(text_scale) * float(unit_to_points))


def _text_anchor(
    entity: dict[str, Any],
    text_em_scale: float = 1.0,
) -> tuple[float, float, str, str]:
    x = float(entity["x"])
    y = float(entity["y"])
    try:
        end_x = float(entity["end_x"])
        end_y = float(entity["end_y"])
        height = _text_em_height(entity, text_em_scale)
    except (KeyError, TypeError, ValueError):
        return x, y, "left", "bottom"

    dx = end_x - x
    dy = end_y - y
    length = math.hypot(dx, dy)
    if length <= 1e-12:
        angle = math.radians(float(entity.get("rotation", 0.0)))
        unit_x = math.cos(angle)
        unit_y = math.sin(angle)
    else:
        unit_x = dx / length
        unit_y = dy / length

    normal_x = -unit_y
    normal_y = unit_x
    center_x = (x + end_x) * 0.5 + normal_x * height * 0.5
    center_y = (y + end_y) * 0.5 + normal_y * height * 0.5
    return center_x, center_y, "center", "center"


def _normalize_polygon_points(
    points: list[tuple[float, float]],
) -> list[tuple[float, float]]:
    if len(points) != 4 or not _polygon_points_cross(points):
        return points

    center_x = sum(point[0] for point in points) / len(points)
    center_y = sum(point[1] for point in points) / len(points)
    ordered = sorted(
        points,
        key=lambda point: math.atan2(point[1] - center_y, point[0] - center_x),
    )
    try:
        index = ordered.index(points[0])
    except ValueError:
        return ordered
    return ordered[index:] + ordered[:index]


def _polygon_points_cross(points: list[tuple[float, float]]) -> bool:
    return _segments_intersect(
        points[0], points[1], points[2], points[3]
    ) or _segments_intersect(points[1], points[2], points[3], points[0])


def _segments_intersect(
    a: tuple[float, float],
    b: tuple[float, float],
    c: tuple[float, float],
    d: tuple[float, float],
) -> bool:
    ab_c = _orientation(a, b, c)
    ab_d = _orientation(a, b, d)
    cd_a = _orientation(c, d, a)
    cd_b = _orientation(c, d, b)
    return ab_c * ab_d < 0.0 and cd_a * cd_b < 0.0


def _orientation(
    a: tuple[float, float],
    b: tuple[float, float],
    c: tuple[float, float],
) -> float:
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0])


def _ellipse_points(
    center_x: float,
    center_y: float,
    major_axis_x: float,
    major_axis_y: float,
    minor_ratio: float,
    start_param: float,
    end_param: float,
) -> list[tuple[float, float]]:
    start = start_param
    end = end_param
    if end <= start:
        end += 2.0 * math.pi
    span = end - start
    count = max(24, int(64 * (span / (2.0 * math.pi))))

    u_x = major_axis_x
    u_y = major_axis_y
    v_x = -major_axis_y * minor_ratio
    v_y = major_axis_x * minor_ratio

    points = []
    for i in range(count + 1):
        t = start + (span * i / count)
        cos_t = math.cos(t)
        sin_t = math.sin(t)
        x = center_x + u_x * cos_t + v_x * sin_t
        y = center_y + u_y * cos_t + v_y * sin_t
        points.append((x, y))
    return points


# ACI display colors for the explicit P4 native reference policy. These are
# viewer defaults, not an RGB palette recovered from the JWC source.
_JWC_ACI_COLORS = {
    132: "#00a5a5",
    18: "#260000",
    92: "#00a500",
    52: "#a5a500",
    212: "#a500a5",
}


def _draw_jwc_text(
    ax: Any, entity: dict[str, Any], color: Any, font: Any, text_scale: float,
    text_em_scale: float = 1.0,
) -> None:
    from matplotlib.patches import PathPatch
    from matplotlib.textpath import TextPath
    from matplotlib.transforms import Affine2D

    content = str(entity.get("content", ""))
    if not content.strip():
        return
    height = _text_em_height(entity, text_em_scale) * float(text_scale)
    path = TextPath((0, 0), content, size=1, prop=font, usetex=False)
    cap_height = (
        TextPath((0, 0), "X", size=1, prop=font, usetex=False).get_extents().height
    )
    scale = height / cap_height
    transform = (
        Affine2D()
        .scale(scale * entity["width_factor"], scale)
        .rotate_deg(float(entity.get("rotation", 0)))
        .translate(float(entity["x"]), float(entity["y"]))
    )
    # Bake glyph coordinates into data units so autoscaling sees the full text.
    ax.add_patch(
        PathPatch(transform.transform_path(path), facecolor=color, edgecolor="none")
    )


def _draw_jww_text(
    ax: Any, entity: dict[str, Any], color: Any, font: Any, text_scale: float,
    text_em_scale: float,
) -> None:
    from matplotlib.patches import PathPatch
    from matplotlib.textpath import TextPath
    from matplotlib.transforms import Affine2D

    content = str(entity.get("content", ""))
    if not content.strip():
        return
    path = TextPath((0, 0), content, size=1, prop=font, usetex=False)
    bounds = path.get_extents()
    x, y, horizontal, vertical = _text_anchor(entity, text_em_scale)
    offset_x = (bounds.x0 + bounds.x1) / 2 if horizontal == "center" else bounds.x0
    offset_y = (bounds.y0 + bounds.y1) / 2 if vertical == "center" else bounds.y0
    height = max(0.0, _text_em_height(entity, text_em_scale) * float(text_scale))
    width = _as_float(entity.get("width_factor"), 1.0)
    if width <= 0.0:
        width = 1.0
    # Apply group 41 along the glyph baseline before rotating into the drawing.
    # TextPath uses an em size; JWC's separate path retains its cap-height policy.
    transform = (
        Affine2D()
        .translate(-offset_x, -offset_y)
        .scale(height * width, height)
        .rotate_deg(_as_float(entity.get("rotation"), 0.0))
        .translate(x, y)
    )
    ax.add_patch(
        PathPatch(transform.transform_path(path), facecolor=color, edgecolor="none")
    )


def plot_dxf_document(
    dxf_document: dict[str, Any],
    *,
    ax: Any | None = None,
    layers: Iterable[str] | None = None,
    linewidth: float = 0.8,
    point_size: float = 1.0,
    draw_text: bool = True,
    draw_points: bool = True,
    draw_inserts: bool = True,
    text_scale: float = 1.0,
    text_em_scale: float = 1.0,
    fill_alpha: float = 0.3,
    draw_fill_edges: bool = False,
    monochrome: bool = False,
    show_axes: bool = False,
    invert_y: bool = False,
    equal_aspect: bool = True,
    autoscale: bool = True,
    show: bool = False,
    save_path: str | Path | None = None,
    dpi: int = 150,
    figsize: tuple[float, float] = (10.0, 10.0),
) -> Any:
    plt, patches = _load_matplotlib()
    layer_filter = _normalize_layer_filter(layers)

    if ax is None:
        fig, ax = plt.subplots(figsize=figsize)
    else:
        fig = ax.figure

    font_properties = _text_font_properties()
    text_kwargs = (
        {"fontproperties": font_properties} if font_properties is not None else {}
    )
    solid_alpha = max(0.0, min(1.0, float(fill_alpha)))
    pending_text: list[tuple[dict[str, Any], Any]] = []
    is_jwc = "jwc_conversion_report" in dxf_document
    hidden_layers = (
        {
            layer["name"]
            for layer in dxf_document.get("layers", [])
            if layer.get("frozen")
        }
        if is_jwc
        else set()
    )
    widths = iter(dxf_document.get("text_width_factors", []))
    patterned_artists: list[tuple[Any, tuple[float, ...], float]] = []

    for entity in dxf_document.get("entities", []):
        if is_jwc and entity.get("type") == "TEXT":
            entity = {**entity, "width_factor": next(widths, 1.0)}
        line_start, patch_start = len(ax.lines), len(ax.patches)
        layer = str(entity.get("layer", "0"))
        if layer in hidden_layers or (
            layer_filter is not None and layer not in layer_filter
        ):
            continue

        entity_type = str(entity.get("type", ""))
        color = _entity_color(int(entity.get("color", 256)), monochrome=monochrome)
        if is_jwc and not monochrome:
            color = _JWC_ACI_COLORS.get(int(entity.get("color", 7)), color)
        line_type = str(entity.get("line_type", "CONTINUOUS"))
        line_style = _line_style(line_type)
        line_capstyle = _line_capstyle(line_type)
        entity_linewidth = _entity_linewidth(entity, linewidth)

        if entity_type == "LINE":
            (line_artist,) = ax.plot(
                [entity["x1"], entity["x2"]],
                [entity["y1"], entity["y2"]],
                color=color,
                linewidth=entity_linewidth,
                linestyle=line_style,
            )
            _apply_line_capstyle(line_artist, line_capstyle)
        elif entity_type == "CIRCLE":
            patch = patches.Circle(
                (entity["center_x"], entity["center_y"]),
                entity["radius"],
                fill=False,
                edgecolor=color,
                linewidth=entity_linewidth,
                linestyle=line_style,
            )
            _apply_line_capstyle(patch, line_capstyle)
            ax.add_patch(patch)
        elif entity_type == "ARC":
            start = float(entity["start_angle"])
            end = float(entity["end_angle"])
            if end < start:
                end += 360.0
            patch = patches.Arc(
                (entity["center_x"], entity["center_y"]),
                2.0 * entity["radius"],
                2.0 * entity["radius"],
                angle=0.0,
                theta1=start,
                theta2=end,
                edgecolor=color,
                linewidth=entity_linewidth,
                linestyle=line_style,
            )
            _apply_line_capstyle(patch, line_capstyle)
            ax.add_patch(patch)
        elif entity_type == "ELLIPSE":
            major_x = float(entity["major_axis_x"])
            major_y = float(entity["major_axis_y"])
            major_radius = math.hypot(major_x, major_y)
            minor_ratio = float(entity["minor_ratio"])
            start_param = float(entity["start_param"])
            end_param = float(entity["end_param"])

            if major_radius <= 0.0:
                continue

            span = end_param - start_param
            if span <= 0.0:
                span += 2.0 * math.pi

            is_full = abs(span - 2.0 * math.pi) < 1e-6
            if is_full:
                angle_deg = math.degrees(math.atan2(major_y, major_x))
                patch = patches.Ellipse(
                    (entity["center_x"], entity["center_y"]),
                    width=2.0 * major_radius,
                    height=2.0 * major_radius * minor_ratio,
                    angle=angle_deg,
                    fill=False,
                    edgecolor=color,
                    linewidth=entity_linewidth,
                    linestyle=line_style,
                )
                _apply_line_capstyle(patch, line_capstyle)
                ax.add_patch(patch)
            else:
                points = _ellipse_points(
                    float(entity["center_x"]),
                    float(entity["center_y"]),
                    major_x,
                    major_y,
                    minor_ratio,
                    start_param,
                    end_param,
                )
                xs, ys = zip(*points)
                (line_artist,) = ax.plot(
                    xs,
                    ys,
                    color=color,
                    linewidth=entity_linewidth,
                    linestyle=line_style,
                )
                _apply_line_capstyle(line_artist, line_capstyle)
        elif entity_type == "POINT":
            if draw_points:
                ax.scatter(
                    [entity["x"]],
                    [entity["y"]],
                    s=point_size,
                    c=[color],
                    marker="o",
                    linewidths=0.0,
                )
        elif entity_type == "TEXT":
            if draw_text:
                if is_jwc:
                    _draw_jwc_text(
                        ax, entity, color, font_properties, text_scale, text_em_scale
                    )
                elif "width_factor" in entity:
                    _draw_jww_text(
                        ax, entity, color, font_properties, text_scale, text_em_scale
                    )
                else:
                    pending_text.append((entity, color))
        elif entity_type == "SOLID":
            points = _normalize_polygon_points(
                [
                    (entity["x1"], entity["y1"]),
                    (entity["x2"], entity["y2"]),
                    (entity["x3"], entity["y3"]),
                    (entity["x4"], entity["y4"]),
                ]
            )
            patch = patches.Polygon(
                points,
                closed=True,
                facecolor=color,
                alpha=solid_alpha,
                **_filled_polygon_edge_kwargs(
                    color,
                    entity_linewidth,
                    draw_edges=draw_fill_edges,
                ),
            )
            ax.add_patch(patch)
        elif entity_type == "FILLED_POLYGON":
            points = [
                (float(point["x"]), float(point["y"]))
                for point in entity.get("points", [])
                if "x" in point and "y" in point
            ]
            if len(points) >= 3:
                patch = patches.Polygon(
                    points,
                    closed=True,
                    facecolor=color,
                    alpha=solid_alpha,
                    **_filled_polygon_edge_kwargs(
                        color,
                        entity_linewidth,
                        draw_edges=draw_fill_edges,
                    ),
                )
                ax.add_patch(patch)
        elif entity_type == "INSERT":
            if draw_inserts:
                x = float(entity.get("x", 0.0))
                y = float(entity.get("y", 0.0))
                ax.scatter([x], [y], s=point_size * 1.5, c=[color], marker="x")
                name = str(entity.get("block_name", ""))
                if name:
                    ax.text(
                        x,
                        y,
                        name,
                        color=color,
                        fontsize=7.0,
                        ha="left",
                        va="bottom",
                        **text_kwargs,
                    )

        if is_jwc and line_type in _JWC_PATTERNS_MM:
            pattern = _JWC_PATTERNS_MM[line_type]
            for artist in list(ax.lines)[line_start:] + list(ax.patches)[patch_start:]:
                patterned_artists.append((artist, pattern, entity_linewidth))

    text_points = []
    for entity, _ in pending_text:
        try:
            text_points.append((float(entity["x"]), float(entity["y"])))
        except (KeyError, TypeError, ValueError):
            continue
    if text_points:
        ax.update_datalim(text_points)

    if equal_aspect:
        ax.set_aspect("equal", adjustable="datalim")
    if autoscale:
        ax.autoscale_view()
    if invert_y:
        ax.invert_yaxis()

    ax.grid(False)
    if show_axes:
        ax.set_xlabel("X")
        ax.set_ylabel("Y")
        ax.set_title("JWC Plot" if is_jwc else "JWW Plot")
    else:
        ax.set_axis_off()

    unit_to_points = _data_unit_to_points(ax)
    for artist, pattern, line_width in patterned_artists:
        # Matplotlib's dash lengths are in points and normally scaled by the
        # stroke width; P4 patterns are explicitly in output millimeters.
        divisor = line_width if plt.rcParams["lines.scale_dashes"] else 1.0
        scale = unit_to_points / max(divisor, 1e-12)
        artist.set_linestyle((0, tuple(length * scale for length in pattern)))
    for entity, color in pending_text:
        content = str(entity.get("content", ""))
        text_x, text_y, horizontal_alignment, vertical_alignment = _text_anchor(
            entity,
            text_em_scale,
        )
        em_height = _text_em_height(entity, text_em_scale)
        ax.text(
            text_x,
            text_y,
            content,
            color=color,
            fontsize=_text_fontsize(em_height, text_scale, unit_to_points),
            rotation=float(entity.get("rotation", 0.0)),
            rotation_mode="anchor",
            ha=horizontal_alignment,
            va=vertical_alignment,
            **text_kwargs,
        )

    if save_path is not None:
        output = Path(save_path)
        output.parent.mkdir(parents=True, exist_ok=True)
        pad_inches = 0.1 if show_axes else 0.0
        fig.savefig(output, dpi=dpi, bbox_inches="tight", pad_inches=pad_inches)
    if show:
        plt.show()

    return ax


def plot_jww(
    path: str | Path,
    *,
    explode_inserts: bool = False,
    max_block_nesting: int = 32,
    text_em_scale: float = 1.0,
    jwc_coordinates: str = "paper_millimeters",
    **kwargs: Any,
) -> Any:
    from ezjww._core import read_dxf_document

    dxf_document = read_dxf_document(
        str(path),
        explode_inserts,
        max_block_nesting,
        text_em_scale,
        jwc_coordinates=jwc_coordinates,
    )
    return plot_dxf_document(dxf_document, text_em_scale=text_em_scale, **kwargs)
