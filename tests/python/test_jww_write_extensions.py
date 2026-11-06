import copy
import importlib.util
from pathlib import Path

import pytest
import ezjww

ROOT = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location(
    "extension_cases", ROOT / "scripts/jww/make_extension_cases.py"
)
cases_module = importlib.util.module_from_spec(spec)
spec.loader.exec_module(cases_module)


def test_extended_cases_roundtrip_and_bounded_conversion(tmp_path):
    for name, drawing in cases_module.cases().items():
        path = tmp_path / (name + ".jww")
        drawing.saveas(path)
        parsed = ezjww.read_document(str(path))
        assert len(parsed["entities"]) == len(drawing.modelspace())
        converted = ezjww.to_write_document(parsed)
        assert converted["document"] is not None, converted["diagnostics"]
        assert any(
            d["code"] == "JWW_TEMPLATE_DEFAULTS" for d in converted["diagnostics"]
        )
        normalized = tmp_path / (name + "_normalized.jww")
        normalized.write_bytes(ezjww.to_jww_bytes(converted["document"]))
        reread = ezjww.read_document(str(normalized))
        assert reread["entities"] == parsed["entities"]
        assert reread["block_defs"] == parsed["block_defs"]
        assert drawing.to_dxf()["entities"]
        if name == "ellipse":
            assert {e["type"] for e in drawing.to_dxf()["entities"]} == {"ELLIPSE"}
        if name == "dimension":
            assert parsed["entities"][0]["line"]["base"]["flag"] == 0x2000
            assert parsed["entities"][0]["text"]["base"]["flag"] == 0x4010
        if name == "solid":
            assert parsed["entities"][1]["color"] == 0x1256AB
            assert parsed["entities"][4]["base"]["pen_style"] == 105
            assert parsed["entities"][5]["base"]["pen_style"] == 106


def test_pen_widths_are_written_and_read_back():
    d = ezjww.new()
    pens = d.options["pens"]
    # the template: Jw_cad's defaults in printer dots at 300 dpi
    assert (pens["width_unit"], pens["max_draw_width"], pens["printer_dpi"]) == ("dot", 100, 300)
    assert [pen["width"] for pen in pens["printer"]] == [1, 1, 2, 3, 4, 5, 6, 7, 8, 1]
    assert len(pens["extended_printer"]) == 257
    pens.update(max_draw_width=-101, width_unit="hundredth_mm", printer_dpi=None)
    for n, pen in enumerate(pens["printer"]):
        pen["width"] = 10 + n
        pen["point_radius"] = 0.25
    pens["printer"][2]["color"] = 0x0000FF
    pens["screen_widths"][3] = 4
    pens["extended_printer"][18] = {"color": 0x00FF00, "width": 35, "point_radius": 0.18}
    d.modelspace().add_line((0, 0), (10, 0))
    back = d.source_document["header"]["pens"]
    assert (back["width_unit"], back["max_draw_width"], back["printer_dpi"]) == (
        "hundredth_mm",
        -101,
        None,
    )
    assert [pen["width"] for pen in back["printer"]] == list(range(10, 20))
    assert back["printer"][2] == {"color": 0x0000FF, "width": 12, "point_radius": 0.25}
    assert back["screen_widths"] == [1, 1, 1, 4, 1, 1, 1, 1, 1, 1]
    assert back["extended_printer"][18] == {"color": 0x00FF00, "width": 35, "point_radius": 0.18}
    assert back["extended_printer"][1]["width"] == 18  # untouched template entry
    # the screen palette is not affected by printer pens
    assert d.source_document["header"]["palette"]["pen_colors"] == d.options["palette"]["pen_colors"]

    # dots at 600 dpi: the flag that records the dpi is rewritten
    d = ezjww.new()
    d.options["pens"]["printer_dpi"] = 600
    assert d.source_document["header"]["pens"]["printer_dpi"] == 600
    d.options["pens"]["printer_dpi"] = None
    assert d.source_document["header"]["pens"]["printer_dpi"] is None
    # no table at all keeps the template's
    d.options["pens"] = None
    assert d.source_document["header"]["pens"]["printer_dpi"] == 300


def test_pen_widths_that_contradict_the_file_are_rejected():
    for change, message in (
        (lambda p: p.update(width_unit="hundredth_mm"), "options.pens.width_unit"),
        (lambda p: p.update(printer_dpi=400), "options.pens.printer_dpi"),
        (lambda p: p.update(extended_printer=p["extended_printer"][:10]), "extended_printer"),
        (lambda p: p["printer"][1].update(point_radius=float("nan")), "point_radius"),
        (lambda p: p.update(unknown=1), "options.pens"),
    ):
        d = ezjww.new()
        change(d.options["pens"])
        with pytest.raises(ValueError, match=message):
            d.to_jww_bytes()


def test_to_write_document_keeps_the_pen_table(tmp_path):
    d = ezjww.new()
    d.options["pens"]["printer"][2]["width"] = 7
    d.modelspace().add_line((0, 0), (10, 0))
    path = tmp_path / "pens.jww"
    d.saveas(path)
    converted = ezjww.to_write_document(ezjww.read_document(str(path)))
    assert converted["document"]["options"]["pens"]["printer"][2]["width"] == 7
    rewritten = tmp_path / "pens2.jww"
    rewritten.write_bytes(ezjww.to_jww_bytes(converted["document"]))
    assert ezjww.read_header(str(rewritten))["pens"]["printer"][2]["width"] == 7


def test_color_allocation_respects_existing_pens_and_overflow():
    d = ezjww.new()
    d.options["palette"]["extended_colors"][17] = 0x123456
    m = d.modelspace()
    m.add_line((0, 0), (1, 1), jwwattribs={"pen_color": 117})
    for color in (0x123456, 0x987654, 0x987654):
        m.add_line((0, 0), (1, 1), color=color)
    parsed = d.source_document
    pens = [e["base"]["pen_color"] for e in parsed["entities"]]
    assert pens == [117, 117, 118, 118]
    assert parsed["header"]["palette"]["extended_colors"][17:19] == [0x123456, 0x987654]
    for pen in range(119, 357):
        m.add_line((0, 0), (1, 1), jwwattribs={"pen_color": pen})
    # Original dictionaries still carry color, so reserve 118 explicitly too.
    m.add_line((0, 0), (1, 1), jwwattribs={"pen_color": 118})
    m.add_line((0, 0), (1, 1), color=0x234567)
    with pytest.raises(ValueError, match="no unused SXF"):
        d.to_jww_bytes()


def test_colors_get_pens_in_document_order_while_entities_are_decoded_one_by_one():
    # Block definitions take free slots before entities, and a pen used further
    # down the entity list is never given to another color.
    d = ezjww.new()
    m = d.modelspace()
    m.add_line((0, 0), (1, 1), color=0x1D2E3F)
    m.add_line((0, 0), (1, 1), jwwattribs={"pen_color": 118})
    scratch = ezjww.new().modelspace()
    d.add_block("B", [scratch.add_line((0, 0), (1, 0), color=0x2A3B4C)])
    parsed = d.source_document
    assert parsed["block_defs"][0]["entities"][0]["base"]["pen_color"] == 117
    assert [e["base"]["pen_color"] for e in parsed["entities"]] == [119, 118]
    colors = parsed["header"]["palette"]["extended_colors"]
    assert (colors[17], colors[19]) == (0x2A3B4C, 0x1D2E3F)


def test_unconvertible_values_are_reported_before_field_errors():
    d = ezjww.new()
    m = d.modelspace()
    m.add_line((0, 0), (1, 1))["unexpected"] = 1
    m.add_line((0, 0), (2, 2))["start_x"] = object()
    with pytest.raises(ValueError, match=r"^document\.entities\[1\]\.start_x: expected a dict"):
        d.to_jww_bytes()


def test_shared_bases_write_the_same_drawing():
    def build(share_bases):
        d = ezjww.new(share_bases=share_bases)
        m = d.modelspace()
        attribs = {"layer": 2, "pen_color": 3}
        entities = [
            m.add_line((0, 0), (1, 1), jwwattribs=attribs),
            m.add_line((0, 0), (2, 2), jwwattribs=attribs),
            m.add_line((0, 0), (3, 3), jwwattribs={"layer": 4}),
            m.add_solid((0, 0), (1, 0), (1, 1), (0, 1), color=0x123456, jwwattribs=attribs),
            m.add_dimension((0, 0), (10, 0), "10", (2, 1), jwwattribs=attribs),
        ]
        return d, entities

    shared, (a, b, c, solid, dimension) = build(True)
    plain, (pa, pb, *_rest) = build(False)
    assert shared.to_jww_bytes() == plain.to_jww_bytes()
    assert a["base"] is b["base"] and a["base"] is not c["base"]
    # A base changed before the entity is added (the solid's color pen) is shared as changed
    assert solid["base"]["pen_color"] == 10 and solid["base"] is not a["base"]
    # Dimension members keep their own base, so callers may still adjust them
    points = dimension["aux_points"]
    assert points[0]["base"] is not points[1]["base"]
    assert pa["base"] is not pb["base"]


def _mixed_drawing(stream_after):
    """A drawing with RGB colors everywhere, its block defined after the entities.

    `stream_after`: None, or how many entities to add before stream_entities().
    """
    d = ezjww.new(share_bases=True)
    m = d.modelspace()
    image = d.embed_image("site.png", b"\x89PNG\r\n\x1a\n" + bytes(range(32)))
    adds = [
        lambda: m.add_line((0, 0), (1, 1), color=0x1D2E3F),
        lambda: m.add_line((0, 0), (2, 2), jwwattribs={"pen_color": 118}),
        lambda: m.add_arc((5, 5), 2.0, 10.0, 80.0, color=0x2A3B4C),
        lambda: m.add_circle((5, 5), 1.0, color=0x000000),
        lambda: m.add_point((3, 4), color=0x0A0B0C),
        lambda: m.add_text("寸法 10", (1, 2), color=0x1D2E3F),
        lambda: m.add_solid((0, 0), (1, 0), (1, 1), (0, 1), color=0x445566),
        lambda: m.add_dimension(
            (0, 0),
            (10, 0),
            "10",
            (2, 1),
            text_options={"jwwattribs": {"pen_style": 1}},
            jwwattribs={"pen_style": 2, "layer": 3},
        ),
        lambda: m.add_ellipse((20, 20), 6.0, 0.5, tilt_angle=30.0, color=0x3E3F40),
        lambda: m.add_image(image, (30, 30), 10.0, 5.0),
        lambda: m.add_block_ref(0, (40, 40)),
    ]
    for i, add in enumerate(adds):
        if i == stream_after:
            d.stream_entities()
        add()
    # The block's colors take their pens first although it is defined last
    block = ezjww.JwwModelspace(d.options)
    block.add_line((0, 0), (5, 5), color=0x0A0B0C)
    block.add_line((0, 0), (5, 0), color=0x5A5B5C)
    d.add_block("B", block.entities)
    return d


@pytest.mark.parametrize("stream_after", [0, 4])
def test_streamed_entities_write_the_same_drawing(stream_after):
    streamed = _mixed_drawing(stream_after)
    assert streamed.to_jww_bytes() == _mixed_drawing(None).to_jww_bytes()
    assert streamed.modelspace().entities == []
    assert streamed.refused_entities == []


def test_streamed_entities_leave_out_what_the_writer_refuses():
    def build(stream):
        d = ezjww.new()
        if stream:
            d.stream_entities()
        m = d.modelspace()
        m.add_line((0, 0), (1, 1), color=0x1D2E3F)
        if stream:
            m.add_line((0, 0), (2, 2), jwwattribs={"pen_color": 357})
            m.add_line((0, 0), (float("nan"), 2))
        m.add_line((0, 0), (3, 3), color=0x2A3B4C)
        return d

    streamed = build(True)
    assert streamed.to_jww_bytes() == build(False).to_jww_bytes()
    (first, reason), (second, conversion) = streamed.refused_entities
    assert (first, second) == (1, 2)
    assert reason.startswith("entities[1].base.pen_color: expected pen color")
    assert conversion == "document.entities[2].end_x: expected a finite number"


def test_streamed_drawing_is_written_once_and_finish_can_be_retried():
    def build(pen):
        d = ezjww.new()
        if pen is None:
            d.stream_entities()
        d.modelspace().add_line((0, 0), (1, 1), color=0x1D2E3F)
        block = ezjww.JwwModelspace(d.options)
        block.add_line((0, 0), (1, 0), jwwattribs={"pen_color": 357 if pen is None else pen})
        d.add_block("B", block.entities)
        return d

    d = build(None)
    with pytest.raises(ValueError, match=r"^block_defs\[0\]\.entities\[0\]\.base\.pen_color"):
        d.to_jww_bytes()
    d.block_defs[0]["entities"][0]["base"]["pen_color"] = 2
    assert d.to_jww_bytes() == build(2).to_jww_bytes()
    with pytest.raises(ValueError, match="takes no more entities"):
        d.modelspace().add_line((0, 0), (1, 1))
    with pytest.raises(ValueError, match="already finished"):
        d.to_jww_bytes()
    with pytest.raises(ValueError, match="streamed"):
        d.source_document


def test_dimension_points_keep_pen_style_one():
    d = ezjww.new()
    dimension = d.modelspace().add_dimension(
        (0, 0),
        (10, 0),
        "10",
        (2, 1),
        text_options={"jwwattribs": {"pen_style": 1}},
        jwwattribs={"pen_style": 2},
    )
    assert dimension["line"]["base"]["pen_style"] == 2
    assert [p["base"]["pen_style"] for p in dimension["aux_points"]] == [1, 1, 1, 1]
    d.to_jww_bytes()  # the points' pen style 2 was refused before


@pytest.mark.parametrize("color", [10, 16, 99, 357])
def test_reserved_color_numbers_are_rejected_for_lines(color):
    d = ezjww.new()
    d.modelspace().add_line((0, 0), (1, 1), jwwattribs={"pen_color": color})
    with pytest.raises(ValueError, match="pen_color"):
        d.to_jww_bytes()


@pytest.mark.parametrize(
    "change",
    [
        lambda d: d["options"]["palette"]["extended_colors"].pop(),
        lambda d: d["options"]["palette"]["pen_colors"].__setitem__(1, 0x1000000),
        lambda d: d["options"]["line_types"]["standard"][0].update(printer_pitch=0),
        lambda d: d["options"]["line_types"]["sxf"][17].update(segments_mm=[2, -1]),
        lambda d: d["options"]["line_types"]["sxf"][17].update(segments_mm=[1] * 12),
        lambda d: d["options"]["line_types"]["sxf"][17].update(extra=1),
        lambda d: d["options"]["text_presets"].pop(),
        lambda d: d["options"]["text_presets"][0].update(size_x=0),
    ],
)
def test_header_table_validation(change):
    d = ezjww.new_jww_document()
    change(d)
    with pytest.raises(ValueError):
        ezjww.to_jww_bytes(d)


def test_ellipse_solid_dimension_and_block_invalid_inputs():
    d = ezjww.new()
    m = d.modelspace()
    m.add_solid((0, 0), (1, 1), (2, 2))
    with pytest.raises(ValueError, match="degenerate"):
        d.to_jww_bytes()
    m.entities.clear()
    m.add_circle_solid((0, 0), 10, inner_radius=10)
    with pytest.raises(ValueError, match="inner radius"):
        d.to_jww_bytes()
    m.entities.clear()
    m.add_ellipse((0, 0), 10, 1.1)
    with pytest.raises(ValueError, match="flatness"):
        d.to_jww_bytes()
    m.entities.clear()
    dimension = m.add_dimension((0, 0), (10, 0), "10", (2, 1))
    dimension["aux_lines"].pop()
    with pytest.raises(ValueError, match="aux_lines"):
        d.to_jww_bytes()
    m.entities.clear()
    m.add_block_ref(999, (0, 0))
    with pytest.raises(ValueError, match="missing block"):
        d.to_jww_bytes()
    m.entities.clear()
    ref = m.add_block_ref(0, (0, 0))
    d.add_block("cycle", [ref])
    with pytest.raises(ValueError, match="cyclic"):
        d.to_jww_bytes()


def test_bulk_and_layer_selection_and_auto_text():
    d = ezjww.new()
    d.select_layer(2, 7)
    m = d.modelspace()
    line = m.add_line((0, 0), (1, 1))
    assert line["base"]["layer_group"] == 2 and line["base"]["layer"] == 7
    m.extend([copy.deepcopy(line) for _ in range(1000)])
    assert len(d.source_document["entities"]) == 1001
    d.select_layer(4, 3)
    assert d.options["layer_groups"][2]["state"] == 2
    d.select_layer(2, 1)
    assert d.options["layer_groups"][2]["layers"][7]["state"] == 2
    text = m.add_text("日Aｶ", (0, 0), size_x=4, spacing=1, angle=90)
    assert text["end_x"] == pytest.approx(0)
    assert text["end_y"] == pytest.approx(10)
    preset = d.options["text_presets"][2]
    preset.update(size_x=5, size_y=6, spacing=1, pen_color=102)
    text = m.add_text("AB", (0, 0), text_type=3)
    assert (
        text["size_x"],
        text["size_y"],
        text["spacing"],
        text["base"]["pen_color"],
    ) == (5, 6, 1, 102)
    assert d.header["text_presets"][2] == preset
    lines = m.add_multiline_text("AB\n日本", (0, 0), line_spacing=8)
    assert lines[1]["start_y"] == -8
    with pytest.raises(ValueError):
        d.select_layer(True, 0)


def test_conversion_lists_each_unsupported_entity_without_silent_omission():
    d = ezjww.new()
    d.modelspace().add_line((0, 0), (1, 1))
    parsed = d.source_document
    parsed["entities"].extend([{"type": "IMAGE"}, {"type": "UNKNOWN"}])
    strict = ezjww.to_write_document(parsed)
    assert strict["document"] is None
    errors = [e for e in strict["diagnostics"] if e["code"] == "JWW_UNSUPPORTED_ENTITY"]
    assert [e["path"] for e in errors] == ["entities[1]", "entities[2]"]
    subset = ezjww.to_write_document(parsed, skip_unsupported=True)
    assert len(subset["document"]["entities"]) == 1
    assert all(
        e["action"] == "omitted"
        for e in subset["diagnostics"]
        if e["severity"] == "error"
    )
    ezjww.to_jww_bytes(subset["document"])


def test_explicit_degenerate_skip_font_factors_and_old_dimension_diagnostics():
    drawing = ezjww.new()
    m = drawing.modelspace()
    with pytest.warns(UserWarning, match="zero-area"):
        assert m.add_solid((0, 0), (1, 1), (2, 2), degenerate="warn_skip") is None
    assert len(m) == 0
    m.font_width_factors["test-font"] = 1.25
    t = m.add_text("AB", (0, 0), font_name="test-font", size_x=4)
    assert t["end_x"] == 5
    m.add_dimension((0, 0), (10, 0), "10", (2, 1))
    parsed = drawing.source_document
    del parsed["entities"][1]["line"]["base"]
    result = ezjww.to_write_document(parsed)
    assert result["document"] is None
    assert "attributes are missing" in result["diagnostics"][-1]["message"]
    m.entities.clear()
    # Unequal crossing lobes have nonzero signed area but remain invalid.
    m.add_solid((0, 0), (3, 3), (0, 3), (1, 0))
    with pytest.raises(ValueError, match="self-crossing"):
        drawing.to_jww_bytes()


def test_color_is_rejected_on_block_and_inside_base():
    drawing = ezjww.new()
    line = drawing.modelspace().add_line((0, 0), (1, 1))
    line["base"]["color"] = 123
    with pytest.raises(ValueError, match="color"):
        drawing.to_jww_bytes()
    del line["base"]["color"]
    drawing.add_block("part", [line])
    drawing.modelspace().add_block_ref("part", (0, 0))["color"] = 123
    with pytest.raises(ValueError, match="color"):
        drawing.to_jww_bytes()


def test_native_block_name_and_reader_diagnostics():
    drawing = ezjww.new()
    drawing.add_block("part@@SfigorgFlag@@4", [])
    drawing.modelspace().add_block_ref(0, (0, 0))
    parsed = drawing.source_document
    assert ezjww.to_write_document(parsed)["document"] is not None
    parsed["diagnostics"] = [
        {"code": "ENTITY_LIST_TRUNCATED", "severity": "error", "message": "incomplete"}
    ]
    result = ezjww.to_write_document(parsed, skip_unsupported=True)
    assert result["document"] is None
    assert any(d["code"] == "JWW_READER_DIAGNOSTIC" for d in result["diagnostics"])


def test_add_block_can_take_ownership_of_the_entity_list():
    # copy_entities=False skips the deep copy: the block keeps the given dictionaries
    # (the list itself is still ours), while the default keeps copying
    import ezjww

    d = ezjww.new()
    owned = [{"kind": "probe", "points": [[0.0, 0.0], [5.0, 5.0]]}]
    copied = d.add_block("copied", owned)
    taken = d.add_block("taken", owned, copy_entities=False)
    assert copied != taken
    defs = {b["name"]: b for b in d.block_defs}
    assert defs["copied"]["entities"] == owned
    assert defs["copied"]["entities"][0] is not owned[0]
    assert defs["taken"]["entities"] == owned
    assert defs["taken"]["entities"] is not owned
    assert defs["taken"]["entities"][0] is owned[0]
