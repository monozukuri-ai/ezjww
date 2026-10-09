use super::{JwwWriteDocument, JwwWriteError};
use crate::model::{
    metadata_setting_from_text, parse_image_reference, EmbeddedImage, Entity, EntityBase,
    IMAGE_TEXT_PREFIX,
};
use std::f64::consts::TAU;

type Result = std::result::Result<(), JwwWriteError>;

fn require(valid: bool, field: &str, reason: &str) -> Result {
    if valid {
        Ok(())
    } else {
        Err(JwwWriteError::invalid(field, reason))
    }
}

fn finite(value: f64, field: &str) -> Result {
    require(value.is_finite(), field, "expected a finite number")
}

fn string(value: &str, field: &str) -> Result {
    require(!value.contains('\0'), field, "NUL is not supported")
}

pub(super) fn document(document: &JwwWriteDocument) -> Result {
    let options = &document.options;
    if options.version != 700 {
        return Err(JwwWriteError::UnsupportedVersion(options.version));
    }
    require(
        matches!(options.paper_size, 0..=4 | 8..=14),
        "options.paper_size",
        "expected 0..=4 or 8..=14",
    )?;
    string(&options.memo, "options.memo")?;
    require(
        options.write_layer_group < 16,
        "options.write_layer_group",
        "expected 0..=15",
    )?;
    for (g, group) in options.layer_groups.iter().enumerate() {
        let path = format!("options.layer_groups[{g}]");
        require(
            group.state <= 3 && (group.state == 3) == (g == options.write_layer_group as usize),
            &format!("{path}.state"),
            "expected 0..=3 with only the current group in state 3",
        )?;
        require(
            group.write_layer < 16,
            &format!("{path}.write_layer"),
            "expected 0..=15",
        )?;
        require(
            group.scale.is_finite() && group.scale > 0.0,
            &format!("{path}.scale"),
            "expected a finite positive scale denominator",
        )?;
        require(
            group.protect <= 2,
            &format!("{path}.protect"),
            "expected 0..=2",
        )?;
        string(&group.name, &format!("{path}.name"))?;
        for (l, layer) in group.layers.iter().enumerate() {
            let path = format!("{path}.layers[{l}]");
            require(
                layer.state <= 3 && (layer.state == 3) == (l == group.write_layer as usize),
                &format!("{path}.state"),
                "expected 0..=3 with only the current layer in state 3",
            )?;
            require(
                layer.protect <= 2,
                &format!("{path}.protect"),
                "expected 0..=2",
            )?;
            string(&layer.name, &format!("{path}.name"))?;
        }
    }
    tables(options)?;
    blocks(document)?;
    images(&document.images)?;
    for (index, item) in document.entities.iter().enumerate() {
        let path = format!("entities[{index}]");
        entity(item, &path, options)?;
        image_reference(item, &path, &document.images)?;
    }
    for (i, block) in document.block_defs.iter().enumerate() {
        for (j, item) in block.entities.iter().enumerate() {
            let path = format!("block_defs[{i}].entities[{j}]");
            entity(item, &path, options)?;
            image_reference(item, &path, &document.images)?;
        }
    }
    Ok(())
}

fn images(images: &[EmbeddedImage]) -> Result {
    let mut names = std::collections::HashSet::new();
    for (i, image) in images.iter().enumerate() {
        let path = format!("images[{i}]");
        string(&image.name, &format!("{path}.name"))?;
        require(
            !image.name.is_empty() && !image.name.contains(['/', '\\']),
            &format!("{path}.name"),
            "expected a bare file name",
        )?;
        require(
            names.insert(image.name.to_lowercase()),
            &format!("{path}.name"),
            "duplicate image name",
        )?;
        require(
            !image.data.is_empty(),
            &format!("{path}.data"),
            "expected image bytes",
        )?;
        if u32::try_from(image.data.len()).is_err() {
            return Err(JwwWriteError::LimitExceeded {
                field: format!("{path}.data"),
                maximum: u64::from(u32::MAX),
            });
        }
        if image.is_compressed() {
            require(
                image.data.starts_with(&[0x1f, 0x8b]),
                &format!("{path}.data"),
                "a .gz image must hold gzip data",
            )?;
        }
    }
    Ok(())
}

/// An image placement that points into the archive (`%temp%`) must name an
/// embedded image; Jw_cad would otherwise show an empty frame. External paths
/// are links the user keeps alongside the drawing and pass unchanged.
fn image_reference(entity: &Entity, path: &str, images: &[EmbeddedImage]) -> Result {
    let Entity::Text(text) = entity else {
        return Ok(());
    };
    let Some(reference) = text.image_reference() else {
        return Ok(());
    };
    if !reference.is_embedded() {
        return Ok(());
    }
    require(
        images.iter().any(|image| {
            image
                .reference_name()
                .eq_ignore_ascii_case(&reference.file_name)
        }),
        &format!("{path}.content"),
        &format!(
            "image {} is not embedded; add it with images / add_image",
            reference.file_name
        ),
    )
}

pub(super) fn entity(entity: &Entity, path: &str, _options: &super::JwwWriteOptions) -> Result {
    let check = |field: &str, value: f64| finite(value, &format!("{path}.{field}"));
    let valid =
        |field: &str, ok: bool, reason: &str| require(ok, &format!("{path}.{field}"), reason);
    match entity {
        Entity::Line(line) => {
            base(&line.base, path, false, false)?;
            for (field, value) in [
                ("start_x", line.start_x),
                ("start_y", line.start_y),
                ("end_x", line.end_x),
                ("end_y", line.end_y),
            ] {
                check(field, value)?;
            }
        }
        Entity::Arc(arc) => {
            base(&arc.base, path, false, false)?;
            valid(
                "base.flag",
                arc.base.flag == 0,
                "arc attributes are unsupported",
            )?;
            for (field, value) in [
                ("center_x", arc.center_x),
                ("center_y", arc.center_y),
                ("radius", arc.radius),
                ("start_angle", arc.start_angle),
                ("arc_angle", arc.arc_angle),
                ("tilt_angle", arc.tilt_angle),
                ("flatness", arc.flatness),
            ] {
                check(field, value)?;
            }
            valid("radius", arc.radius > 0.0, "expected a positive radius")?;
            valid(
                "flatness",
                arc.flatness > 0.0 && arc.flatness <= 1.0,
                "expected flatness in (0, 1]",
            )?;
            if arc.is_full_circle {
                valid(
                    "start_angle",
                    arc.start_angle == 0.0,
                    "full circles must start at zero",
                )?;
                valid(
                    "arc_angle",
                    arc.arc_angle == TAU,
                    "full circles must sweep 2*pi radians",
                )?;
            } else {
                valid(
                    "start_angle",
                    (0.0..TAU).contains(&arc.start_angle),
                    "expected radians in [0, 2*pi)",
                )?;
                valid(
                    "arc_angle",
                    arc.arc_angle > 0.0 && arc.arc_angle < TAU,
                    "expected a counterclockwise sweep in (0, 2*pi) radians",
                )?;
            }
        }
        Entity::Point(point) => {
            base(&point.base, path, true, false)?;
            check("x", point.x)?;
            check("y", point.y)?;
            valid(
                "is_temporary",
                !point.is_temporary,
                "temporary points are not supported",
            )?;
            valid("code", point.code == 0, "marker points are not supported")?;
            valid(
                "angle",
                point.angle == 0.0,
                "ordinary points have no stored angle",
            )?;
            valid(
                "scale",
                point.scale == 0.0,
                "ordinary points have no stored scale; use zero",
            )?;
        }
        Entity::Text(text) => {
            base(&text.base, path, false, true)?;
            for (field, value) in [
                ("start_x", text.start_x),
                ("start_y", text.start_y),
                ("end_x", text.end_x),
                ("end_y", text.end_y),
                ("size_x", text.size_x),
                ("size_y", text.size_y),
                ("spacing", text.spacing),
                ("angle", text.angle),
            ] {
                check(field, value)?;
            }
            valid(
                "size_x",
                text.size_x > 0.0,
                "expected a positive character width",
            )?;
            valid(
                "size_y",
                text.size_y > 0.0,
                "expected a positive character height",
            )?;
            valid(
                "spacing",
                text.spacing >= 0.0,
                "expected nonnegative spacing",
            )?;
            valid(
                "angle",
                (0.0..360.0).contains(&text.angle),
                "expected degrees in [0, 360)",
            )?;
            valid(
                "text_type",
                text.text_type <= 30010 && text.text_type % 10000 <= 10,
                "expected text type 0..=10 plus 10000/20000 style flags",
            )?;
            for (field, value) in [("font_name", &text.font_name), ("content", &text.content)] {
                string(value, &format!("{path}.{field}"))?;
                valid(
                    field,
                    !value.chars().any(char::is_control),
                    "expected a single line without control characters",
                )?;
            }
            valid(
                "font_name",
                !text.font_name.is_empty(),
                "expected a font name",
            )?;
            if text.content.starts_with(IMAGE_TEXT_PREFIX) {
                let reference = parse_image_reference(&text.content);
                valid(
                    "content",
                    reference.is_some(),
                    "expected ^@BM<file>,<width>,<height>[,...] for an image placement",
                )?;
                let reference = reference.expect("checked");
                valid(
                    "content",
                    reference.width > 0.0 && reference.height > 0.0,
                    "expected a positive image width and height in millimetres",
                )?;
            } else {
                valid(
                    "content",
                    !text.content.starts_with("^@"),
                    "embedded control text is not supported",
                )?;
            }
            valid(
                "content",
                metadata_setting_from_text(text).is_none(),
                "native internal setting text is not supported",
            )?;
        }
        Entity::Solid(s) => {
            solid_base(&s.base, s.color, path)?;
            valid(
                "base.pen_style",
                s.base.pen_style <= 100,
                "polygon solid style must be <= 100",
            )?;
            let points = [
                (s.point1_x, s.point1_y),
                (s.point4_x, s.point4_y),
                (s.point2_x, s.point2_y),
                (s.point3_x, s.point3_y),
            ];
            for (i, &(x, y)) in points.iter().enumerate() {
                check(&format!("point{}_x", [1, 4, 2, 3][i]), x)?;
                check(&format!("point{}_y", [1, 4, 2, 3][i]), y)?;
            }
            let area = (0..4)
                .map(|i| {
                    let (x, y) = points[i];
                    let (a, b) = points[(i + 1) % 4];
                    x * b - y * a
                })
                .sum::<f64>();
            valid(
                "points",
                area.is_finite()
                    && area != 0.0
                    && !crosses(points[0], points[1], points[2], points[3])
                    && !crosses(points[1], points[2], points[3], points[0]),
                "degenerate or self-crossing solids are not supported",
            )?;
        }
        Entity::CircleSolid(s) => {
            solid_base(&s.base, s.color, path)?;
            for (field, value) in [
                ("center_x", s.center_x),
                ("center_y", s.center_y),
                ("radius", s.radius),
                ("flatness", s.flatness),
                ("tilt_angle", s.tilt_angle),
                ("start_angle", s.start_angle),
                ("arc_angle", s.arc_angle),
                ("solid_mode", s.solid_mode),
            ] {
                check(field, value)?;
            }
            valid("radius", s.radius > 0.0, "expected a positive radius")?;
            valid(
                "flatness",
                s.flatness > 0.0 && s.flatness <= 1.0,
                "expected flatness in (0, 1]",
            )?;
            valid(
                "start_angle",
                (0.0..TAU).contains(&s.start_angle),
                "expected radians in [0, 2*pi)",
            )?;
            valid(
                "arc_angle",
                s.arc_angle > 0.0 && s.arc_angle <= TAU,
                "expected sweep in (0, 2*pi]",
            )?;
            let mode_ok = match s.base.pen_style {
                101 => {
                    matches!(s.solid_mode, -1.0 | 0.0 | 5.0 | 100.0)
                        && (s.solid_mode != -1.0 || s.arc_angle <= TAU / 4.0)
                        && (s.solid_mode != 100.0 || s.arc_angle == TAU)
                }
                105 | 106 => {
                    s.solid_mode > 0.0
                        && s.solid_mode < s.radius
                        && (s.base.pen_style != 106
                            || s.radius - s.solid_mode < s.radius * s.flatness)
                }
                111 => s.solid_mode == 0.0 || (s.solid_mode == 100.0 && s.arc_angle == TAU),
                _ => false,
            };
            valid(
                "solid_mode",
                mode_ok,
                "unsupported circle solid style/mode or inner radius",
            )?;
        }
        Entity::Block(b) => {
            base(&b.base, path, false, false)?;
            for (field, value) in [
                ("ref_x", b.ref_x),
                ("ref_y", b.ref_y),
                ("scale_x", b.scale_x),
                ("scale_y", b.scale_y),
                ("rotation", b.rotation),
            ] {
                check(field, value)?;
            }
            valid(
                "scale",
                b.scale_x != 0.0 && b.scale_y != 0.0,
                "block scales must be nonzero",
            )?;
        }
        Entity::Dimension(d) => {
            base(&d.base, path, false, false)?;
            valid(
                "sxf_mode",
                matches!(d.sxf_mode, Some(0 | 1)),
                "expected SXF mode 0 or 1",
            )?;
            valid(
                "aux_lines",
                d.aux_lines.len() == 2,
                "expected two auxiliary lines",
            )?;
            valid(
                "aux_points",
                d.aux_points.len() == 4,
                "expected four auxiliary points",
            )?;
            self::entity(
                &Entity::Line(d.line.clone()),
                &format!("{path}.line"),
                _options,
            )?;
            self::entity(
                &Entity::Text(d.text.clone()),
                &format!("{path}.text"),
                _options,
            )?;
            for (i, line) in d.aux_lines.iter().enumerate() {
                self::entity(
                    &Entity::Line(line.clone()),
                    &format!("{path}.aux_lines[{i}]"),
                    _options,
                )?;
            }
            for (i, point) in d.aux_points.iter().enumerate() {
                self::entity(
                    &Entity::Point(point.clone()),
                    &format!("{path}.aux_points[{i}]"),
                    _options,
                )?;
            }
        }
    }
    Ok(())
}

fn crosses(a: (f64, f64), b: (f64, f64), c: (f64, f64), d: (f64, f64)) -> bool {
    let side = |p: (f64, f64), q: (f64, f64), r: (f64, f64)| {
        (q.0 - p.0) * (r.1 - p.1) - (q.1 - p.1) * (r.0 - p.0)
    };
    let opposite = |x: f64, y: f64| (x > 0.0 && y < 0.0) || (x < 0.0 && y > 0.0);
    opposite(side(a, b, c), side(a, b, d)) && opposite(side(c, d, a), side(c, d, b))
}

fn base(base: &EntityBase, path: &str, point: bool, text: bool) -> Result {
    let path = format!("{path}.base");
    require(
        base.group == 0
            && if text {
                matches!(base.flag, 0 | 0x10 | 0x4000 | 0x4010)
            } else if point {
                matches!(base.flag, 0 | 0x40)
            } else {
                matches!(base.flag, 0 | 0x2000)
            },
        &path,
        "only named dimension attribute flags are supported; curve groups are unsupported",
    )?;
    require(base.layer < 16, &format!("{path}.layer"), "expected 0..=15")?;
    require(
        base.layer_group < 16,
        &format!("{path}.layer_group"),
        "expected 0..=15",
    )?;
    require(
        matches!(base.pen_color, 1..=9 | 100..=356),
        &format!("{path}.pen_color"),
        "expected pen color 1..=9 or 100..=356 (SXF); 10 is reserved for solid RGB",
    )?;
    if text || point {
        require(
            base.pen_style == 1,
            &format!("{path}.pen_style"),
            "only the default text anchor or ordinary point style (1) is supported",
        )?;
    } else {
        require(
            matches!(base.pen_style, 1..=9 | 16..=19 | 30..=62),
            &format!("{path}.pen_style"),
            "expected line style 1..=9, 16..=19 or 30..=62",
        )?;
    }
    if text {
        require(
            (base.pen_width == 0 || (base.flag & 0x4010 != 0 && base.pen_width & !0x3ffa == 0))
                && base.pen_width & 0x300 != 0x300
                && base.pen_width & 0xc00 != 0xc00,
            &format!("{path}.pen_width"),
            "expected zero or a supported dimension text flag combination",
        )?;
    } else {
        require(
            base.pen_width <= 500,
            &format!("{path}.pen_width"),
            "expected native width code 0..=500",
        )?;
    }
    Ok(())
}

fn solid_base(value: &EntityBase, color: Option<u32>, path: &str) -> Result {
    let mut copy = *value;
    copy.pen_style = 1;
    if color.is_some() {
        copy.pen_color = 1;
    }
    base(&copy, path, false, false)?;
    require(
        value.flag == 0,
        &format!("{path}.base.flag"),
        "solid attributes are unsupported",
    )?;
    require(
        (value.pen_color == 10) == color.is_some(),
        &format!("{path}.color"),
        "solid RGB requires pen_color 10 and vice versa",
    )?;
    require(
        color.is_none_or(|v| v <= 0xffffff),
        &format!("{path}.color"),
        "expected COLORREF 0..=0xffffff",
    )
}

fn tables(options: &super::JwwWriteOptions) -> Result {
    if let Some(p) = &options.palette {
        require(
            p.extended_colors.as_ref().is_some_and(|v| v.len() == 257),
            "options.palette.extended_colors",
            "expected 257 colors",
        )?;
        require(
            p.pen_colors
                .iter()
                .chain(p.extended_colors.iter().flatten())
                .all(|v| *v <= 0xffffff),
            "options.palette",
            "expected COLORREF 0..=0xffffff",
        )?;
    }
    if let Some(t) = &options.line_types {
        require(
            t.standard.len() == 8
                && t.random.len() == 5
                && t.double_length.len() == 4
                && t.sxf.as_ref().is_some_and(|v| v.len() == 33),
            "options.line_types",
            "expected 8 standard, 5 random, 4 double_length and 33 SXF slots",
        )?;
        for (items, first) in [(&t.standard, 2), (&t.double_length, 16)] {
            for (i, p) in items.iter().enumerate() {
                require(
                    p.number == first + i as u32,
                    "options.line_types.number",
                    "slot numbers must be in order",
                )?;
                pattern(p)?;
            }
        }
        for (i, p) in t.random.iter().enumerate() {
            require(
                p.number == 11 + i as u32 && p.pitch > 0 && p.printer_pitch > 0,
                "options.line_types.random",
                "invalid slot number or pitch",
            )?;
        }
        for (i, p) in t.sxf.as_ref().unwrap().iter().enumerate() {
            require(
                p.pattern.number == 30 + i as u32,
                "options.line_types.sxf.number",
                "expected ordered slots 30..=62",
            )?;
            pattern(&p.pattern)?;
            string(&p.name, "options.line_types.sxf.name")?;
            require(
                p.segments_mm.len() <= 10
                    && p.segments_mm.len() % 2 == 0
                    && p.segments_mm.iter().all(|v| v.is_finite() && *v > 0.0),
                "options.line_types.sxf.segments_mm",
                "expected zero or up to ten positive alternating dash/gap lengths",
            )?;
        }
    }
    if let Some(p) = &options.text_presets {
        require(
            p.len() == 10,
            "options.text_presets",
            "expected ten presets",
        )?;
        for p in p {
            require(
                p.size_x.is_finite()
                    && p.size_x > 0.0
                    && p.size_y.is_finite()
                    && p.size_y > 0.0
                    && p.spacing.is_finite()
                    && p.spacing >= 0.0
                    && matches!(p.pen_color,1..=9|100..=356),
                "options.text_presets",
                "invalid character size, spacing or pen color",
            )?;
        }
    }
    Ok(())
}
fn pattern(p: &crate::header::LineTypePattern) -> Result {
    require(
        (p.number == 30 && p.unit_dots == 0)
            || ((1..=32).contains(&p.unit_dots) && p.pitch > 0 && p.printer_pitch > 0),
        "options.line_types",
        "expected unit_dots 0..=32 and positive pitches",
    )
}

fn blocks(document: &JwwWriteDocument) -> Result {
    use std::collections::{HashMap, HashSet};
    require(
        document.block_defs.len() <= 10000,
        "block_defs",
        "at most 10000 definitions",
    )?;
    let mut defs = HashMap::new();
    let mut names = HashSet::new();
    let mut references = HashSet::new();
    for (i, d) in document.block_defs.iter().enumerate() {
        require(
            defs.insert(d.number, i).is_none() && names.insert(&d.name),
            "block_defs",
            "duplicate definition number or name",
        )?;
        string(&d.name, "block_defs.name")?;
        require(
            !d.name.is_empty()
                && (!d.name.contains("@@SfigorgFlag@@")
                    || d.name
                        .strip_suffix("@@SfigorgFlag@@4")
                        .is_some_and(|s| !s.is_empty() && !s.contains("@@SfigorgFlag@@"))),
            "block_defs.name",
            "expected a nonempty block name; only native SfigorgFlag 4 is supported",
        )?;
        base(&d.base, "block_defs", false, false)?;
    }
    let mut edges = vec![Vec::new(); document.block_defs.len()];
    for (owner, entities) in std::iter::once((None, &document.entities)).chain(
        document
            .block_defs
            .iter()
            .enumerate()
            .map(|(i, d)| (Some(i), &d.entities)),
    ) {
        for entity in entities {
            if let Entity::Block(b) = entity {
                let Some(&target) = defs.get(&b.def_number) else {
                    return Err(JwwWriteError::invalid(
                        "block.def_number",
                        "missing block definition",
                    ));
                };
                references.insert(b.def_number);
                if let Some(owner) = owner {
                    edges[owner].push(target);
                }
            }
        }
    }
    fn depth(
        i: usize,
        edges: &[Vec<usize>],
        active: &mut [bool],
        memo: &mut [usize],
        level: usize,
    ) -> std::result::Result<usize, JwwWriteError> {
        if active[i] || level > 32 {
            return Err(JwwWriteError::invalid(
                "block_defs",
                "cyclic reference or nesting exceeds 32",
            ));
        }
        if memo[i] > 0 {
            return Ok(memo[i]);
        }
        active[i] = true;
        let mut result = 1;
        for &child in &edges[i] {
            result = result.max(1 + depth(child, edges, active, memo, level + 1)?);
        }
        active[i] = false;
        memo[i] = result;
        require(result <= 32, "block_defs", "nesting exceeds 32")?;
        Ok(result)
    }
    let mut active = vec![false; edges.len()];
    let mut memo = vec![0; edges.len()];
    for (i, d) in document.block_defs.iter().enumerate() {
        require(
            d.is_referenced == references.contains(&d.number),
            "block_defs.is_referenced",
            "must agree with references in the document",
        )?;
        depth(i, &edges, &mut active, &mut memo, 1)?;
    }
    Ok(())
}
