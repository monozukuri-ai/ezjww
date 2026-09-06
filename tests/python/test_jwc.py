import json
import shutil
from pathlib import Path

import pytest

import ezjww

ROOT = Path(__file__).resolve().parents[2]
SAMPLES = ROOT / "jwc_samples/generated"


def sample(name="q032"):
    path = SAMPLES / f"{name}.jwc"
    assert path.is_file(), f"required JWC fixture missing: {path}"
    return str(path)


@pytest.mark.parametrize("name", ["q070", "r001", "r013"])
def test_truncated_inputs_do_not_return_partial_documents(name, tmp_path):
    data = Path(sample(name)).read_bytes()
    path = tmp_path / "truncated.jwc"
    for end in [0, 12, 200, 2420, 2421, len(data) - 1]:
        path.write_bytes(data[:end])
        for read in [ezjww.read_jwc_document, ezjww.read_cad_document, ezjww.readfile]:
            with pytest.raises(ValueError):
                read(str(path))


def test_source_document_contract_and_legacy_drawing_constructor():
    path = sample()
    assert ezjww.is_jwc_file(path) and not ezjww.is_jww_file(path)
    assert ezjww.detect_file_format(path) == "jwc"
    document = ezjww.read_jwc_document(path)
    assert document["entities"][0]["content"] == "日本語"
    assert document["entity_counts"] == {"TEXT": 1}
    assert document["header"] == ezjww.read_jwc_header(path)
    assert document["header"]["source_version"] is None
    assert "version" not in document["header"]
    assert ezjww.read_cad_document(path) == {"format": "jwc", "document": document}
    drawing = ezjww.readfile(path)
    assert drawing.source_format == "jwc"
    assert drawing.source_document == document
    assert drawing.jww_document is None
    assert drawing.header == document["header"]
    legacy = {"header": {"version": 700}}
    drawing = ezjww.Drawing(source_path=None, jww_document=legacy)
    assert drawing.source_format == "jww"
    assert drawing.source_document is drawing.jww_document is legacy
    empty = ezjww.new()
    assert empty.source_format is empty.source_document is empty.jww_document is None


def test_jww_only_readers_and_content_detection(tmp_path):
    renamed = tmp_path / "renamed.JWW"
    shutil.copyfile(sample(), renamed)
    assert ezjww.readfile(renamed).source_format == "jwc"
    with pytest.raises(ValueError):
        ezjww.read_document(str(renamed))
    with pytest.raises(ValueError):
        ezjww.read_header(str(renamed))
    native = str(SAMPLES / "q032rt.jww")
    assert ezjww.read_cad_document(native) == {
        "format": "jww",
        "document": ezjww.read_document(native),
    }
    with pytest.raises(ValueError):
        ezjww.read_jwc_document(native)
    unknown = tmp_path / "unknown.jwc"
    unknown.write_bytes(b"unrecognized")
    assert ezjww.detect_file_format(str(unknown)) is None
    with pytest.raises(ValueError, match="unrecognized CAD format"):
        ezjww.readfile(unknown)
    with pytest.raises(OSError):
        ezjww.read_jwc_header(str(tmp_path / "missing.jwc"))


@pytest.mark.parametrize("name,offset", [("r011", 2441), ("r080", 2483)])
def test_unsupported_inputs_fail_all_document_and_conversion_readers(name, offset):
    path = sample(name)
    assert ezjww.is_jwc_file(path)
    for read in [
        ezjww.read_jwc_document,
        ezjww.read_cad_document,
        ezjww.read_dxf_document,
        ezjww.read_dxf_string,
        ezjww.readfile,
        ezjww.report,
    ]:
        with pytest.raises(ValueError, match=f"byte {offset}"):
            read(path)


def test_coordinate_space_reaches_drawing_stats_bbox_and_writers(tmp_path):
    path = sample("q054")
    paper = ezjww.readfile(path)
    model = ezjww.Drawing.from_file(path, jwc_coordinates="model_millimeters")
    a, b = paper.modelspace().entities[0], model.modelspace().entities[0]
    assert b["x1"] == pytest.approx(a["x1"] * 50)
    assert model.bbox()["width"] == pytest.approx(paper.bbox()["width"] * 50)
    assert model.stats()["by_type"] == {"LINE": 1}
    assert (
        model.report()["jwc_conversion_report"]["coordinate_space"]
        == "model_millimeters"
    )
    output = tmp_path / "scaled.dxf"
    model.saveas(output, target_version="AC1024")
    assert output.read_text() == model.to_dxf_string(target_version="AC1024")
    assert output.read_text() == ezjww.to_dxf_string(
        path, target_version="AC1024", jwc_coordinates="model_millimeters"
    )
    with pytest.raises(ValueError):
        ezjww.read_dxf_document(path, jwc_coordinates="invalid")
    with pytest.raises(ValueError):
        ezjww.read_dxf_string(path, max_block_nesting=0)


def test_dxf_metadata_width_and_write_report_are_preserved(tmp_path):
    path = sample("r014")
    dxf = ezjww.read_dxf_document(path)
    assert dxf["text_width_factors"] == pytest.approx([3 / 4.2])
    assert dxf["jwc_conversion_report"]["notices"]
    output = tmp_path / "output.dxf"
    report = ezjww.write_dxf_with_report(path, str(output), target_version="AC1024")
    assert report["source_version"] is None
    assert report["source_format"] == "jwc"
    assert report["source_entities"] == report["converted_entities"] == 1
    assert report["converted_entity_counts"] == {"TEXT": 1}
    assert report["jwc_conversion_report"] == dxf["jwc_conversion_report"]
    assert output.read_text() == ezjww.read_dxf_string(path, target_version="AC1024")
    target = tmp_path / "plain.dxf"
    ezjww.write_dxf(path, str(target))
    assert target.read_text() == ezjww.read_dxf_string(path)


def test_cp932_diagnostics_reach_audit_and_report(tmp_path):
    source = ezjww.read_jwc_document(sample("q030"))
    string = source["entities"][0]["string_source"]
    offset = string["byte_offset"] + string["byte_length"] - 2
    data = bytearray(Path(sample("q030")).read_bytes())
    data[offset] = 0x81  # Incomplete CP932 lead byte before the existing NUL.
    path = tmp_path / "replacement.jwc"
    path.write_bytes(data)
    diagnostic = ezjww.read_jwc_document(str(path))["diagnostics"][0]
    assert diagnostic["code"] == "CP932_DECODE_REPLACED"
    audit = ezjww.audit(path)
    assert audit["decode_error_count"] == 1 and audit["has_issues"]
    assert diagnostic in audit["diagnostics"]
    assert ezjww.report(path)["audit"] == audit
    assert diagnostic in audit["jwc_conversion_report"]["diagnostics"]


@pytest.mark.parametrize("coordinates", ["paper_millimeters", "model_millimeters"])
@pytest.mark.parametrize("target", ["AC1015", "AC1024"])
def test_text_scale_preserves_jwc_width_policy_and_writes_one_factor(tmp_path, coordinates, target):
    source = sample("r014")
    options = dict(jwc_coordinates=coordinates, target_version=target, text_em_scale=1.364)
    plain = ezjww.read_dxf_document(source, jwc_coordinates=coordinates)
    scaled = ezjww.read_dxf_document(source, jwc_coordinates=coordinates, text_em_scale=1.364)
    a, b = plain["entities"][0], scaled["entities"][0]
    assert b["height"] == pytest.approx(a["height"] / 1.364)
    assert b["width_factor"] == pytest.approx(3 / 4.2)
    assert scaled["text_width_factors"] == pytest.approx([b["width_factor"]])
    assert scaled["jwc_conversion_report"] == plain["jwc_conversion_report"]
    drawing = ezjww.readfile(source, jwc_coordinates=coordinates)
    assert drawing.to_dxf(text_em_scale=1.364) == scaled
    text = ezjww.read_dxf_string(source, **options)
    record, = [r for r in text.split("  0\n") if r.startswith("TEXT\n")]
    lines = record.splitlines()[1:]
    widths = [float(value) for code, value in zip(lines[::2], lines[1::2]) if code.strip() == "41"]
    assert widths == pytest.approx([3 / 4.2])
    assert drawing.to_dxf_string(target_version=target, text_em_scale=1.364) == text
    for writer in (ezjww.write_dxf, ezjww.write_dxf_with_report):
        output = tmp_path / f"{writer.__name__}.dxf"
        writer(source, str(output), **options)
        assert output.read_text() == text


@pytest.mark.parametrize("command", ["info", "audit", "bbox", "stats", "report"])
def test_cli_read_entrypoints(command, capsys):
    assert ezjww._run([command, sample(), "--json"]) == 0
    value = json.loads(capsys.readouterr().out)
    assert value
    if command in {"info", "audit", "report"}:
        assert value["source_format"] == "jwc"


def test_cli_batch_handles_mixed_extensions_and_preflights_collisions(tmp_path, capsys):
    inputs = tmp_path / "inputs"
    inputs.mkdir()
    shutil.copyfile(sample(), inputs / "a.JwC")
    shutil.copyfile(SAMPLES / "q032rt.jww", inputs / "b.JWW")
    output = tmp_path / "success"
    assert ezjww._run(["to-dxf-dir", str(inputs), "-o", str(output)]) == 0
    assert (output / "a.dxf").is_file() and (output / "b.dxf").is_file()
    shutil.copyfile(SAMPLES / "q032rt.jww", inputs / "a.jww")
    collision_output = tmp_path / "collision"
    assert ezjww._run(["to-dxf-dir", str(inputs), "-o", str(collision_output)]) == 2
    assert not collision_output.exists()
    assert "output collision" in capsys.readouterr().err


def test_cli_single_and_plot_entrypoints(tmp_path):
    pytest.importorskip("matplotlib")
    import matplotlib.pyplot as plt

    path = sample()
    dxf = tmp_path / "single.dxf"
    png = tmp_path / "single.png"
    assert ezjww._run(["to-dxf", path, "-o", str(dxf)]) == 0
    assert dxf.read_text() == ezjww.read_dxf_string(path)
    assert ezjww._run(["plot", path, "-o", str(png)]) == 0
    assert png.stat().st_size > 1000
    visible = ezjww.readfile(path).plot()
    assert len(visible.patches) > 0  # Japanese text path, not an insertion marker.
    hidden = ezjww.plot_jww(sample("q053"))
    assert not hidden.lines and not hidden.patches and not hidden.collections
    plt.close("all")
