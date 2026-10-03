#!/usr/bin/env python3
"""Verify the recorded Jw_cad writer matrix without launching the application."""

from __future__ import annotations

import argparse
import gzip
import hashlib
import json
import math
import tempfile
from collections import Counter
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
FIXTURES = ROOT / "jww_samples/writer/compatibility"
CASES = (
    "empty",
    "line",
    "circle",
    "arc",
    "point",
    "text",
    "basic",
    "settings",
    "unicode",
    "attributes",
    "text_styles",
    "text_rotations",
    "count65534",
    "count65535",
    "count65536",
    "late_classes",
    "cstring_boundaries",
    "memo_ascii64",
    "memo_ascii65",
    "memo_utf16_64",
    "memo_utf16_65",
)
SETTING_KEYS = {
    "Printer_Orientation",
    "Printer_PaperSize",
    "Printer_D2dBMP",
    "Printer_BmpZENTAI",
    "View_Direct2d",
    "Draw_BmpTOUKA",
}
EXE_SHA256 = "95e6b11c4ee014e0079f288429ae2c6e5eed41141a963e4b8770d2e8ead87acf"
DXF_HEADER_SHA256 = "78d8cfb8c1f630078205a0d612b98c82075e350801412d8fc282820253ffc430"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return dict(size=len(data), sha256=hashlib.sha256(data).hexdigest())


def artifact(directory, manifest, name):
    require(Path(name).name == name, "artifact names must be basenames")
    record = manifest["artifacts"][name]
    path = record["path"]
    require(Path(path).name == path, "artifact paths must be basenames")
    packed = (directory / path).read_bytes()
    require(digest(packed) == record["stored"], "changed archive: " + path)
    data = gzip.decompress(packed) if path.endswith(".gz") else packed
    require(digest(data) == record["original"], "changed artifact: " + name)
    return data


def check_integrity(directory=FIXTURES):
    manifest = json.loads((directory / "manifest.json").read_text(encoding="utf-8"))
    require(manifest["schema_version"] == 1, "unknown compatibility schema")
    require(manifest["application_version"] == "10.02.1", "unverified Jw_cad version")
    require(manifest["executable_sha256"] == EXE_SHA256, "unverified executable")
    require(
        manifest["dxf_header_sha256"] == DXF_HEADER_SHA256, "unverified DXF template"
    )
    require(
        [case["id"] for case in manifest["cases"]] == list(CASES),
        "changed case inventory",
    )
    require(
        manifest["windows_validation"] == "not_run",
        "Windows evidence must be recorded separately",
    )
    for name in manifest["artifacts"]:
        artifact(directory, manifest, name)
    events = {}
    for name in manifest["runs"]:
        run = json.loads(artifact(directory, manifest, name))
        script = artifact(directory, manifest, manifest["run_scripts"][name])
        if "script_sha256" in run:
            require(
                digest(script)["sha256"] == run["script_sha256"],
                "changed native runner",
            )
        require(run["executable_sha256"] == EXE_SHA256, "run used another executable")
        require(run["environment"]["kind"] == "wine", "wrong runtime in Wine evidence")
        require(run["environment"]["wine_version"] == "9.0", "unverified Wine version")
        require(run["environment"]["ansi_code_page"] == 932, "unexpected code page")
        for event in run["cases"]:
            require(event["id"] not in events, "duplicate native run case")
            events[event["id"]] = event
            require(
                event["status"] in {"save_reopen_save", "save_reopen_save_export"},
                "native case failed",
            )
            require(
                event["input_unchanged"] and event["exit_codes"] == [0, 0],
                "incomplete native execution",
            )
            for name, expected in event["files"].items():
                require(
                    digest(artifact(directory, manifest, name)) == expected,
                    "run artifact mismatch: " + name,
                )
    require(set(events) == set(CASES), "missing native execution")
    for case in manifest["cases"]:
        name = case["id"]
        event = events[name]
        stages = ["opened", "saved", "reopened", "resaved"]
        if case["native_dxf"]:
            stages.append("dxf_exported")
        require(event["stages"] == stages, "unexpected native stages: " + name)
        expected_files = {
            name + suffix for suffix in (".jww", "_saved.jww", "_reopened.jww")
        }
        if case["native_dxf"]:
            expected_files.add(name + "_reopened.dxf")
        require(
            expected_files <= event["files"].keys(), "missing native output: " + name
        )
    display_name = manifest["display_run"]
    display = json.loads(artifact(directory, manifest, display_name))
    require(display["executable_sha256"] == EXE_SHA256, "unverified display executable")
    require(
        display["environment"]["kind"] == "wine"
        and display["environment"]["wine_version"] == "9.0",
        "unverified display runtime",
    )
    require(
        digest(artifact(directory, manifest, manifest["run_scripts"][display_name]))[
            "sha256"
        ]
        == display["script_sha256"],
        "changed display runner",
    )
    require(
        [event["id"] for event in display["cases"]] == ["circle", "basic"],
        "incomplete display probe",
    )
    for event in display["cases"]:
        name = event["id"]
        require(
            event["input_unchanged"] and event["exit_code"] == 0,
            "display execution failed",
        )
        require(
            set(event["files"])
            == {name + s for s in ("_reopened.jww", "_gdi.jww", "_gdi.bmp")},
            "missing display artifact",
        )
        for filename, expected in event["files"].items():
            require(
                digest(artifact(directory, manifest, filename)) == expected,
                "display artifact mismatch",
            )
        original = artifact(directory, manifest, name + "_reopened.jww")
        old, new = (
            s.encode("utf-16-le") for s in ("View_Direct2d = 1", "View_Direct2d = 0")
        )
        require(original.count(old) == 1, "missing Direct2D setting")
        require(
            artifact(directory, manifest, name + "_gdi.jww")
            == original.replace(old, new),
            "display probe changed drawing data",
        )
    return manifest


def is_setting(entity):
    return (
        entity["type"] == "TEXT"
        and entity["start_x"] == entity["end_x"] == 0
        and entity["start_y"] == entity["end_y"] == -1000
        and entity["content"].split("=", 1)[0].strip() in SETTING_KEYS
    )


def close(actual, expected):
    if isinstance(expected, dict):
        return actual.keys() == expected.keys() and all(
            close(actual[k], v) for k, v in expected.items()
        )
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(
            close(a, b) for a, b in zip(actual, expected)
        )
    if isinstance(expected, float):
        return math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-10)
    return actual == expected


def compare_native(source, saved, case):
    name = case["id"]
    require(
        len(source["entities"]) == case["input_entities"],
        name + ": changed input count",
    )
    require(not saved["diagnostics"], name + ": native read diagnostics")
    require(not saved["block_defs"], name + ": unexpected blocks")
    for field in (
        "version",
        "paper_size",
        "write_layer_group",
        "layer_groups",
        "palette",
    ):
        require(
            saved["header"][field] == source["header"][field],
            name + ": changed header " + field,
        )
    require(
        saved["header"]["memo"] == case["native_memo"],
        name + ": unexpected native memo",
    )
    settings = [e for e in saved["entities"] if is_setting(e)]
    require(
        Counter(e["content"].split("=", 1)[0].strip() for e in settings)
        == Counter(SETTING_KEYS),
        name + ": changed internal setting inventory",
    )
    actual = [e for e in saved["entities"] if not is_setting(e)]
    expected = [
        e for e in source["entities"] if not (e["type"] == "TEXT" and not e["content"])
    ]
    require(len(actual) == len(expected), name + ": lost or added geometry")
    text_index = 0
    for i, (a, b) in enumerate(zip(actual, expected)):
        a = a.copy()
        if b["type"] in {"ARC", "CIRCLE"}:
            require(
                math.isclose(
                    math.sin(a["start_angle"]),
                    math.sin(b["start_angle"]),
                    abs_tol=1e-12,
                )
                and math.isclose(
                    math.cos(a["start_angle"]),
                    math.cos(b["start_angle"]),
                    abs_tol=1e-12,
                ),
                f"{name}: changed arc start at {i}",
            )
            a["start_angle"] = b["start_angle"]
        if b["type"] == "TEXT":
            measured = case["native_text_endpoints"][text_index]
            require(
                close([a["end_x"], a["end_y"]], measured),
                f"{name}: changed text endpoint at {i}",
            )
            a["end_x"], a["end_y"] = b["end_x"], b["end_y"]
            text_index += 1
        require(close(a, b), f"{name}: changed geometry or attributes at {i}")
    require(
        text_index == len(case["native_text_endpoints"]),
        name + ": unused text expectation",
    )
    return dict(
        entities=len(expected),
        internal_settings=len(settings),
        empty_text_removed=len(source["entities"]) - len(expected),
        memo_preserved=saved["header"]["memo"] == source["header"]["memo"],
        memo_text_preserved=saved["header"]["memo"].replace("\r\n", "")
        == source["header"]["memo"].replace("\r\n", ""),
        memo_utf16_units=len(saved["header"]["memo"].encode("utf-16-le")) // 2,
        changed_header_fields=[
            k for k in source["header"] if saved["header"][k] != source["header"][k]
        ],
    )


def read_dxf_tags(data):
    lines = data.decode("cp932").splitlines()
    require(len(lines) % 2 == 0, "incomplete native DXF tags")
    tags = [
        (int(lines[i].strip()), lines[i + 1].strip()) for i in range(0, len(lines), 2)
    ]
    require(tags[-1] == (0, "EOF"), "incomplete native DXF")
    start = tags.index((2, "ENTITIES")) + 1
    end = tags.index((0, "ENDSEC"), start)
    entities = []
    for code, value in tags[start:end]:
        if code == 0:
            entities.append({0: value})
        else:
            require(code not in entities[-1], "duplicate native DXF entity tag")
            entities[-1][code] = value
    return entities


def compare_dxf(source, data, case):
    """Compare native DXF tags directly; do not use ezjww's DXF converter."""
    tags = read_dxf_tags(data)
    expected = [
        e for e in source["entities"] if not (e["type"] == "TEXT" and not e["content"])
    ]
    require(len(tags) == len(expected), case["id"] + ": native DXF count mismatch")
    scale, offset_x, offset_y = case["native_dxf_transform"]
    for i, (a, b) in enumerate(zip(tags, expected)):
        require(a[0] == b["type"], f"{case['id']}: native DXF type at {i}")
        require(
            {str(code): a.get(code) for code in (8, 6, 62)}
            == case["native_dxf_attributes"][i],
            f"{case['id']}: native DXF layer/line type/color at {i}",
        )
        pairs = {
            "LINE": [(10, "start_x"), (20, "start_y"), (11, "end_x"), (21, "end_y")],
            "CIRCLE": [(10, "center_x"), (20, "center_y")],
            "ARC": [(10, "center_x"), (20, "center_y")],
            "POINT": [(10, "x"), (20, "y")],
            "TEXT": [(10, "start_x"), (20, "start_y")],
        }[b["type"]]
        for code, field in pairs:
            offset = offset_y if field.endswith("y") else offset_x
            require(
                math.isclose(float(a[code]), (b[field] + offset) * scale, abs_tol=1e-7),
                f"{case['id']}: native DXF coordinate {i}.{code}",
            )
        if b["type"] in {"ARC", "CIRCLE"}:
            require(
                math.isclose(float(a[40]), b["radius"] * scale, abs_tol=1e-7),
                "native DXF radius",
            )
        if b["type"] == "ARC":
            for code, angle in [
                (50, b["start_angle"]),
                (51, b["start_angle"] + b["arc_angle"]),
            ]:
                require(
                    math.isclose(
                        float(a[code]), math.degrees(angle) % 360, abs_tol=1e-7
                    ),
                    "native DXF angle",
                )
        if b["type"] == "TEXT":
            require(a[1] == b["content"], "native DXF text content")
            require(
                math.isclose(float(a[40]), b["size_y"] * scale, abs_tol=1e-7),
                "native DXF text height",
            )
            require(
                math.isclose(float(a[41]), b["size_x"] / b["size_y"], abs_tol=1e-7),
                "native DXF text width",
            )
            # Jw_cad's DXF exporter uses the supplied baseline direction. Its
            # on-screen text rotation uses the separate JWW angle field.
            angle = (
                math.degrees(
                    math.atan2(b["end_y"] - b["start_y"], b["end_x"] - b["start_x"])
                )
                % 360
            )
            require(
                math.isclose(float(a[50]) % 360, angle, abs_tol=1e-7),
                "native DXF text rotation",
            )
    return len(tags)


def check(directory=FIXTURES, generated=None):
    import ezjww

    manifest = check_integrity(directory)
    results = []
    with tempfile.TemporaryDirectory(prefix="ezjww-native-check-") as temp:
        temp = Path(temp)
        for case in manifest["cases"]:
            name = case["id"]
            inputs = {}
            for suffix in (".jww", "_saved.jww", "_reopened.jww"):
                filename = name + suffix
                data = artifact(directory, manifest, filename)
                path = temp / filename
                path.write_bytes(data)
                inputs[suffix] = ezjww.read_document(str(path))
                require(
                    not inputs[suffix]["diagnostics"], filename + ": reader diagnostics"
                )
                if suffix == ".jww" and generated:
                    require(
                        (generated / filename).read_bytes() == data,
                        "generator changed: " + filename,
                    )
            source = inputs[".jww"]
            result = {"id": name}
            for suffix in ("_saved.jww", "_reopened.jww"):
                result[suffix] = compare_native(source, inputs[suffix], case)
            if case["native_dxf"]:
                result["native_dxf_entities"] = compare_dxf(
                    source, artifact(directory, manifest, name + "_reopened.dxf"), case
                )
            results.append(result)
    return dict(
        application_version=manifest["application_version"],
        runtime="wine-9.0",
        windows_validation="not_run",
        cases=results,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fixtures", type=Path, default=FIXTURES)
    parser.add_argument(
        "--generated",
        type=Path,
        help="Compare newly generated Rust inputs byte for byte",
    )
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    result = check(args.fixtures, args.generated)
    if args.report:
        args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(
        f"Native writer compatibility: {len(result['cases'])} recorded Wine cases verified; Windows not run."
    )


if __name__ == "__main__":
    main()
