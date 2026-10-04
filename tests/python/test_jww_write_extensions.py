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
