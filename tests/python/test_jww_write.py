from __future__ import annotations

import copy
import math
from pathlib import Path
from unittest.mock import patch

import pytest

import ezjww
from ezjww import _core

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "jww_samples" / "writer"


def acceptance(name):
    doc = ezjww.new()
    msp = doc.modelspace()
    if name == "empty":
        return doc
    if name == "line":
        msp.add_line((0, 0), (100, 0))
    elif name == "basic":
        msp.add_line(
            (-50, -20),
            (50, -20),
            jwwattribs={
                "pen_style": 3,
                "pen_color": 3,
                "pen_width": 25,
                "layer": 2,
            },
        )
        msp.add_circle((-30, 20), 10, jwwattribs={"pen_color": 2})
        msp.add_arc((0, 20), 10, 350, 30)
        msp.add_point((30, 20), jwwattribs={"pen_color": 4})
        msp.add_text(
            "日本語 ABC",
            (-50, -40),
            (-20, -40),
            spacing=0.5,
            jwwattribs={"pen_color": 5},
        )
        msp.add_text("縦方向", (30, -40), (30, -20), angle=90, size_y=5)
    elif name == "settings":
        doc.options.update(paper_size=4, memo="図面設定の検証\r\n", write_layer_group=2)
        for g, group in enumerate(doc.options["layer_groups"]):
            group.update(
                name=f"グループ{g:X}",
                state=3 if g == 2 else g % 3,
                write_layer=5,
                scale=50.0 if g == 2 else 0.5,
                protect=1 if g == 4 else 2 if g == 5 else 0,
            )
            for layer_index, layer in enumerate(group["layers"]):
                layer.update(
                    name=f"層{g:X}-{layer_index:X}",
                    state=3 if layer_index == 5 else layer_index % 3,
                    protect=1 if layer_index == 8 else 2 if layer_index == 9 else 0,
                )
        msp.add_line((-10, -5), (10, 5), jwwattribs={"layer_group": 2, "layer": 5})
        msp.add_circle((0, 0), 2, jwwattribs={"layer_group": 15, "layer": 15})
    elif name == "unicode":
        doc.options["memo"] = "日本語𠮷\r\n"
        doc.options["layer_groups"][0]["name"] = "図面𠮷"
        doc.options["layer_groups"][0]["layers"][2]["name"] = "日本語の層" * 60
        for i, text in enumerate(["", "日本語𠮷ｶﾅ ABC", "長" * 255, "文" * 1024]):
            msp.add_text(text, (0, i * 10), (20, i * 10))
    return doc


@pytest.mark.parametrize("name", ["empty", "line", "basic", "settings", "unicode"])
def test_python_generates_exact_native_qualified_rust_inputs(name, tmp_path):
    doc = acceptance(name)
    fixture = (
        FIXTURES / ("basic" if name not in {"empty", "line"} else "") / f"{name}.jww"
    )
    data = doc.to_jww_bytes()
    assert type(data) is bytes
    assert data == fixture.read_bytes() == doc.to_jww_bytes()
    output = tmp_path / f"{name}.JWW"
    doc.saveas(output)
    assert output.read_bytes() == data
    parsed = ezjww.readfile(output)
    assert parsed.jww_document == doc.jww_document == doc.source_document
    assert parsed.header == doc.header
    assert parsed.to_dxf() == doc.to_dxf()
    assert parsed.to_dxf_string() == doc.to_dxf_string()
    assert parsed.audit()["has_issues"] is False
    assert doc.audit()["has_issues"] is False
    assert len(doc.modelspace()) == len(parsed.modelspace())


def test_factories_have_distinct_contracts():
    for factory in (ezjww.new, ezjww.Drawing.new):
        doc = factory(paper_size=4, memo="生成")
        assert isinstance(doc, ezjww.JwwDrawing)
        assert isinstance(doc.modelspace(), ezjww.JwwModelspace)
        assert doc.source_path is None and doc.source_format == "jww"
        assert doc.header["paper_size"] == 4 and doc.header["memo"] == "生成"
        assert doc.bbox() is None and len(doc.modelspace()) == 0
    for factory in (ezjww.new_dxf, ezjww.Drawing.new_dxf):
        doc = factory()
        assert type(doc) is ezjww.Drawing
        assert (
            doc.source_format
            is doc.source_document
            is doc.jww_document
            is doc.header
            is None
        )
        assert len(doc.modelspace()) == 0 and doc.bbox() is None
        with pytest.raises(ValueError, match="source-backed"):
            doc.to_dxf_string()


@pytest.mark.parametrize(
    "kwargs",
    [
        {"version": 600},
        {"paper_size": 7},
        {"memo": "bad\0memo"},
        {"paper_size": True},
    ],
)
def test_constructor_validates_options(kwargs):
    with pytest.raises(ValueError):
        ezjww.new(**kwargs)


def test_native_geometry_and_query_contract():
    doc = acceptance("basic")
    msp = doc.modelspace()
    assert list(msp) == msp.entities
    assert msp.query('LINE[layer=="0-2", color==3]') == [msp.entities[0]]
    assert msp.query("CIRCLE ARC") == msp.entities[1:3]
    assert msp.query(layer="0-0", color=4) == [msp.entities[3]]
    assert msp.query("SOLID") == []
    assert msp.entities[2]["start_angle"] == pytest.approx(math.radians(350))
    assert msp.entities[2]["arc_angle"] == pytest.approx(math.radians(30))
    assert msp.entities[5]["angle"] == 90
    assert msp.bbox() == doc.bbox(explode_inserts=False) and msp.stats() == doc.stats()
    with pytest.raises(ValueError):
        msp.query("LINE[layer~=5]")
    with pytest.raises(ValueError, match="to_dxf"):
        doc.modelspace(text_em_scale=2)
    with pytest.raises(ValueError, match="attributes"):
        msp.add_line((0, 0), (1, 1), jwwattribs={"color": 3})
    assert len(msp) == 6


def test_mutations_refresh_all_derived_views_and_keep_snapshots_isolated(tmp_path):
    doc = ezjww.new()
    msp = doc.modelspace()
    line = msp.add_line((0, 0), (10, 0))
    assert doc.bbox()["width"] == 10
    snapshot = doc.to_dxf()
    source = doc.source_document
    line["end_x"] = 20
    doc.options["memo"] = "更新"
    doc.options["layer_groups"][0]["scale"] = 50
    assert doc.bbox()["width"] == msp.bbox()["width"] == 20
    assert doc.header["memo"] == "更新"
    assert source["header"]["memo"] == ""
    assert snapshot["entities"][0]["x2"] == 10
    snapshot["entities"].clear()
    source["entities"].clear()
    assert doc.stats()["entity_count"] == 1
    msp.entities = [copy.deepcopy(line), copy.deepcopy(line)]
    assert doc.stats()["entity_count"] == 2
    assert doc.source_document["entity_counts"] == {"LINE": 2}
    path = tmp_path / "current.jww"
    doc.saveas(path)
    assert ezjww.read_document(str(path))["entities"] == msp.entities
    assert doc.report()["stats"]["entity_count"] == 2


@pytest.mark.parametrize("version", ["AC1015", "AC1024"])
def test_new_jww_dxf_export_and_plot_match_saved_input(tmp_path, version):
    doc = acceptance("basic")
    source = tmp_path / "source.jww"
    doc.saveas(source)
    output = tmp_path / "out.DXF"
    doc.save_dxf(output, target_version=version, text_em_scale=1.5)
    assert output.read_text(encoding="utf-8") == ezjww.read_dxf_string(
        str(source),
        target_version=version,
        text_em_scale=1.5,
    )
    assert version in output.read_text(encoding="utf-8")
    import matplotlib.pyplot as plt

    result = doc.plot(show=False)
    assert result is not None
    plt.close("all")


def test_file_backed_native_save_is_explicitly_unsupported(tmp_path):
    for path in (
        ROOT / "jww_samples/Test1.jww",
        ROOT / "jwc_samples/generated/q032.jwc",
    ):
        doc = ezjww.readfile(path)
        output = tmp_path / "existing.jww"
        output.write_bytes(b"keep")
        with pytest.raises(ValueError, match="save_dxf"):
            doc.saveas(output)
        assert output.read_bytes() == b"keep"
        dxf = tmp_path / "export.dxf"
        doc.save_dxf(dxf)
        assert dxf.read_text(encoding="utf-8") == doc.to_dxf_string()
        with pytest.raises(ValueError, match="readfile"):
            ezjww.JwwDrawing.from_file(path)


@pytest.mark.parametrize(
    "method,suffix",
    [("saveas", ".dxf"), ("save_dxf", ".jww"), ("saveas", ""), ("save_dxf", ".txt")],
)
def test_output_format_never_follows_a_misleading_extension(tmp_path, method, suffix):
    path = tmp_path / ("existing" + suffix)
    path.write_bytes(b"keep")
    with pytest.raises(ValueError, match="path"):
        getattr(ezjww.new(), method)(path)
    assert path.read_bytes() == b"keep"
    assert len(list(tmp_path.iterdir())) == 1


def test_bad_data_and_failed_replace_preserve_existing_file_and_remove_temporary(
    tmp_path,
):
    path = tmp_path / "existing.jww"
    path.write_bytes(b"keep")
    doc = ezjww.new()
    circle = doc.modelspace().add_circle((0, 0), -1)
    with pytest.raises(ValueError, match=r"entities\[0\].radius"):
        doc.saveas(path)
    assert path.read_bytes() == b"keep"
    circle["radius"] = 1
    with patch("ezjww.os.replace", side_effect=OSError("injected I/O failure")):
        with pytest.raises(OSError, match="injected"):
            doc.saveas(path)
    assert path.read_bytes() == b"keep"
    assert list(tmp_path.iterdir()) == [path]
    doc.saveas(path)
    assert path.read_bytes().startswith(b"JwwData.")


@pytest.mark.parametrize("bad", [float("nan"), float("inf"), float("-inf")])
def test_nonfinite_inputs_cross_the_python_rust_boundary(bad, tmp_path):
    doc = ezjww.new()
    doc.modelspace().add_line((0, 0), (bad, 1))
    for operation in (
        doc.to_jww_bytes,
        doc.to_dxf,
        doc.to_dxf_string,
        doc.bbox,
        doc.stats,
    ):
        with pytest.raises(ValueError, match=r"entities\[0\].end_x"):
            operation()
    with pytest.raises(ValueError):
        doc.save_dxf(tmp_path / "bad.dxf")
    assert not list(tmp_path.iterdir())


def test_low_level_writer_rejects_missing_unknown_and_unsupported_fields():
    mutations = [
        (lambda d: d.update(header={}), "document.header"),
        (lambda d: d["options"].update(palette={}), "options.palette"),
        (lambda d: d["options"]["layer_groups"].pop(), "options.layer_groups"),
        (lambda d: d["options"]["layer_groups"][0]["layers"].pop(), "layers"),
        (lambda d: d["options"].update(paper_size=2**32), "options.paper_size"),
        (lambda d: d["entities"][0].update(type="INSERT"), "entities[0].type"),
        (lambda d: d["entities"][0].update(unrecognized=1), "entities[0].unrecognized"),
        (lambda d: d["entities"][0].pop("start_x"), "entities[0].start_x"),
        (lambda d: d["entities"][0].update(end_x="3"), "entities[0].end_x"),
        (lambda d: d["entities"][0]["base"].update(layer=16), "entities[0].base.layer"),
        (lambda d: d["entities"][0]["base"].update(layer=-1), "entities[0].base.layer"),
        (
            lambda d: d["entities"][0]["base"].update(layer=True),
            "entities[0].base.layer",
        ),
        (lambda d: d["entities"][0]["base"].update(flag=1), "entities[0].base"),
    ]
    for mutate, field in mutations:
        d = _core.new_jww_document()
        d["entities"] = acceptance("line").modelspace().entities
        mutate(d)
        with pytest.raises(ValueError) as error:
            _core.to_jww_bytes(d)
        assert field in str(error.value)
    with pytest.raises(ValueError, match="header"):
        _core.to_jww_bytes(ezjww.new().source_document)


def test_low_level_boolean_fields_and_marker_attributes_are_strict():
    doc = ezjww.new()
    point = doc.modelspace().add_point((0, 0))
    point["is_temporary"] = 0
    with pytest.raises(ValueError, match="expected bool"):
        doc.to_jww_bytes()
    point["is_temporary"] = False
    point["code"] = 10
    with pytest.raises(ValueError, match="marker"):
        doc.to_jww_bytes()
    doc = ezjww.new()
    circle = doc.modelspace().add_circle((0, 0), 3)
    circle["type"] = "ARC"
    with pytest.raises(ValueError, match="must agree"):
        doc.to_jww_bytes()


@pytest.mark.parametrize("content", ["^@BMimage.bmp", "bad\0text", "two\nlines"])
def test_unsupported_text_is_rejected(content):
    doc = ezjww.new()
    doc.modelspace().add_text(content, (0, 0), (5, 0))
    with pytest.raises(ValueError, match="content"):
        doc.to_jww_bytes()


@pytest.mark.parametrize(
    "options",
    [
        {"max_block_nesting": 0},
        {"text_em_scale": 0},
        {"text_em_scale": float("nan")},
    ],
)
def test_low_level_conversion_rejects_invalid_options(options):
    doc = _core.new_jww_document()
    for convert in (
        _core.jww_write_document_to_dxf,
        _core.jww_write_document_to_dxf_string,
    ):
        with pytest.raises(ValueError):
            convert(doc, **options)


def test_dxf_errors_preserve_existing_file(tmp_path):
    path = tmp_path / "existing.dxf"
    path.write_bytes(b"keep")
    for options in [
        {"target_version": "AC1009"},
        {"text_em_scale": 0},
        {"max_block_nesting": 0},
    ]:
        with pytest.raises(ValueError):
            ezjww.new().save_dxf(path, **options)
        assert path.read_bytes() == b"keep"
    with patch("ezjww.os.replace", side_effect=OSError("injected")):
        with pytest.raises(OSError):
            ezjww.new().save_dxf(path)
    assert path.read_bytes() == b"keep" and list(tmp_path.iterdir()) == [path]


def test_stub_exports_match_the_new_extension_entrypoints():
    # Parsing the stub checks names and syntax without a typing_extensions runtime dependency.
    import ast

    source = (Path(ezjww.__file__).parent / "_core.pyi").read_text(encoding="utf-8")
    functions = {
        n.name: n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef)
    }
    import inspect

    for name in (
        "new_jww_document",
        "to_jww_bytes",
        "jww_write_document_to_document",
        "jww_write_document_to_dxf",
        "jww_write_document_to_dxf_string",
    ):
        fn = getattr(_core, name)
        actual = list(inspect.signature(fn).parameters)
        stub = [a.arg for a in functions[name].args.args]
        assert actual == stub
    assert {"JwwDrawing", "JwwModelspace", "new", "new_dxf"} <= set(ezjww.__all__)
