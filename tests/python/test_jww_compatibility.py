"""Regression checks against the archived native application, including losses."""

import copy
import json
import runpy
from pathlib import Path

import pytest

import ezjww

ROOT = Path(__file__).resolve().parents[2]
ORACLE = runpy.run_path(str(ROOT / "scripts/jww/check_compatibility.py"))
FIXTURES = ORACLE["FIXTURES"]


def test_recorded_native_matrix():
    actual = ORACLE["check"]()
    assert actual == json.loads(
        (FIXTURES / "validation.json").read_text(encoding="utf-8")
    )
    cases = {c["id"]: c for c in actual["cases"]}
    assert cases["count65536"]["_reopened.jww"]["entities"] == 65536
    assert cases["late_classes"]["_reopened.jww"]["entities"] == 32772
    assert cases["unicode"]["_reopened.jww"]["empty_text_removed"] == 1
    assert cases["memo_ascii64"]["_reopened.jww"]["memo_text_preserved"]
    for name in (
        "memo_ascii65",
        "memo_utf16_64",
        "memo_utf16_65",
        "cstring_boundaries",
    ):
        assert not cases[name]["_reopened.jww"]["memo_text_preserved"]


@pytest.fixture
def basic(tmp_path):
    manifest = json.loads((FIXTURES / "manifest.json").read_text(encoding="utf-8"))
    case = next(c for c in manifest["cases"] if c["id"] == "basic")
    docs = []
    for name in ("basic.jww", "basic_reopened.jww"):
        path = tmp_path / name
        path.write_bytes(ORACLE["artifact"](FIXTURES, manifest, name))
        docs.append(ezjww.read_document(str(path)))
    return docs[0], docs[1], case, manifest


@pytest.mark.parametrize(
    "mutation", ["lost_entity", "width", "scale", "text", "endpoint", "memo"]
)
def test_native_comparison_rejects_unrecorded_changes(basic, mutation):
    source, saved, case, _ = basic
    saved = copy.deepcopy(saved)
    if mutation == "lost_entity":
        saved["entities"].pop(0)
    elif mutation == "width":
        saved["entities"][0]["base"]["pen_width"] += 1
    elif mutation == "scale":
        saved["header"]["layer_groups"][0]["scale"] *= 2
    elif mutation in {"text", "endpoint"}:
        entity = next(
            e
            for e in saved["entities"]
            if e["type"] == "TEXT" and e["content"] == "日本語 ABC"
        )
        if mutation == "text":
            entity["content"] = "日本語 ABD"
        else:
            entity["end_x"] += 1
    else:
        saved["header"]["memo"] = "changed"
    with pytest.raises(ValueError):
        ORACLE["compare_native"](source, saved, case)


def test_truncated_native_dxf_is_not_accepted(basic):
    source, _, case, manifest = basic
    data = ORACLE["artifact"](FIXTURES, manifest, "basic_reopened.dxf")
    assert ORACLE["compare_dxf"](source, data, case) == 6
    with pytest.raises(ValueError, match="incomplete native DXF"):
        ORACLE["compare_dxf"](source, data[: data.rfind(b"EOF")], case)


def test_archive_corruption_is_not_accepted(basic, tmp_path):
    _, _, _, manifest = basic
    record = manifest["artifacts"]["basic.jww"]
    (tmp_path / record["path"]).write_bytes(b"damaged")
    with pytest.raises(ValueError, match="changed archive"):
        ORACLE["artifact"](tmp_path, manifest, "basic.jww")
