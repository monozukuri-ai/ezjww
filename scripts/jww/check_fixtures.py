#!/usr/bin/env python3
"""Check writer template/native fixture integrity; optionally extract the header."""
import argparse
import hashlib
import json
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
        if entity[0] != "LINE":
            raise ValueError("unexpected native DXF entity: " + entity[0])
        result.append(dict(type="LINE", start=[float(entity[10]), float(entity[20])],
                           end=[float(entity[11]), float(entity[21])]))
    return result


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
    manifest = json.loads((fixtures / "manifest.json").read_text(encoding="utf-8"))
    if sorted(case["id"] for case in manifest["cases"]) != ["empty", "line"]:
        raise ValueError("expected empty and line native cases")
    count = 0
    for case in manifest["cases"]:
        for name, expected in case["files"].items():
            if Path(name).name != name:
                raise ValueError("fixture names must be basenames")
            data = (fixtures / name).read_bytes()
            if len(data) != expected["size"] or hashlib.sha256(data).hexdigest() != expected["sha256"]:
                raise ValueError("native fixture changed: " + name)
            count += 1
        actual = native_dxf_entities(fixtures / (case["id"] + "_reopened.dxf"))
        if actual != manifest["native_dxf_expectations"][case["id"]]:
            raise ValueError("native DXF geometry mismatch: " + case["id"])
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
