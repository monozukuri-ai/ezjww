#!/usr/bin/env python3
"""Check writer template/native fixture integrity; optionally extract the header."""
import argparse
import hashlib
import json
import math
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
TEMPLATE_SHA256 = "7d66238bef9a66e3030a39a58936ba6aedc2189a3ebe46d582c908bf43622572"


def native_dxf_entities(path):
    """Read raw native DXF tags without using ezjww's converter or parser."""
    lines = path.read_text(encoding="cp932").splitlines()
    tags = [(int(lines[i].strip()), lines[i + 1].strip())
            for i in range(0, len(lines), 2)]
    start = tags.index((2, "ENTITIES")) + 1
    end = tags.index((0, "ENDSEC"), start)
    entities = []
    for code, value in tags[start:end]:
        if code == 0:
            entities.append({0: value})
        else:
            entities[-1][code] = value
    result = []
    for entity in entities:
        kind = entity[0]
        xy = lambda x, y: [float(entity[x]), float(entity[y])]
        if kind == "LINE":
            result.append(dict(type=kind, start=xy(10, 20), end=xy(11, 21)))
        elif kind in ("CIRCLE", "ARC"):
            record = dict(type=kind, center=xy(10, 20), radius=float(entity[40]))
            if kind == "ARC":
                record.update(start_degrees=float(entity[50]), end_degrees=float(entity[51]))
            result.append(record)
        elif kind == "POINT":
            result.append(dict(type=kind, position=xy(10, 20)))
        elif kind == "TEXT":
            result.append(dict(type=kind, position=xy(10, 20), height=float(entity[40]),
                               width_factor=float(entity[41]), angle=float(entity[50]), content=entity[1]))
        else:
            raise ValueError("unexpected native DXF entity: " + kind)
    return result


def equivalent(actual, expected):
    if isinstance(expected, dict):
        return actual.keys() == expected.keys() and all(equivalent(actual[k], v) for k, v in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(equivalent(a, b) for a, b in zip(actual, expected))
    if isinstance(expected, (int, float)):
        return math.isclose(actual, expected, rel_tol=1e-12, abs_tol=1e-12)
    return actual == expected


def check_native(fixtures, case_ids):
    manifest = json.loads((fixtures / "manifest.json").read_text(encoding="utf-8"))
    if sorted(case["id"] for case in manifest["cases"]) != sorted(case_ids):
        raise ValueError("unexpected native case inventory")
    count = 0
    for case in manifest["cases"]:
        name = case["id"]
        has_dxf = case["status"] == "save_reopen_save_export"
        if not has_dxf and case["status"] != "save_reopen_save":
            raise ValueError("unexpected native case status")
        suffixes = [".jww", "_saved.jww", "_reopened.jww"] + (["_reopened.dxf"] if has_dxf else [])
        if set(case["files"]) != {name + suffix for suffix in suffixes} or case["exit_codes"] != [0, 0]:
            raise ValueError("incomplete native case: " + name)
        for filename, expected in case["files"].items():
            if Path(filename).name != filename:
                raise ValueError("fixture names must be basenames")
            data = (fixtures / filename).read_bytes()
            if len(data) != expected["size"] or hashlib.sha256(data).hexdigest() != expected["sha256"]:
                raise ValueError("native fixture changed: " + filename)
            count += 1
        if has_dxf:
            actual = native_dxf_entities(fixtures / (name + "_reopened.dxf"))
            if not equivalent(actual, manifest["native_dxf_expectations"][name]):
                raise ValueError("native DXF geometry mismatch: " + name)
        elif name in manifest["native_dxf_expectations"]:
            raise ValueError("DXF expectations without a native export: " + name)
    return count


def check():
    source = (ROOT / "jwc_samples/inputs/q000.jww").read_bytes()
    if source[-8:] != bytes(8):
        raise ValueError("q000.jww no longer has the expected empty lists/trailer")
    header = source[:-8]
    if len(header) != 16558 or hashlib.sha256(header).hexdigest() != TEMPLATE_SHA256:
        raise ValueError("header template source changed")
    embedded = ROOT / "crates/ezjww-core/src/writer/templates/header_700.bin"
    if embedded.read_bytes() != header:
        raise ValueError("embedded header differs from its recorded source")
    fixtures = ROOT / "jww_samples/writer"
    count = check_native(fixtures, ["empty", "line"])
    count += check_native(fixtures / "basic", ["basic", "settings", "unicode"])
    print(f"Writer template and {count} native validation files verified.")
    return header


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--extract-header", type=Path)
    args = parser.parse_args()
    header = check()
    if args.extract_header is not None:
        with args.extract_header.open("xb") as output:
            output.write(header)


if __name__ == "__main__":
    main()
