#!/usr/bin/env python3
"""Verify the required original JWC corpus without regenerating native outputs."""

import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check(root):
    corpus = root / "jwc_samples"
    manifest = json.loads((corpus / "manifest.json").read_text(encoding="utf-8"))
    assert manifest["license"] == "MIT; repository LICENSE"
    samples = manifest["samples"]
    assert len(samples) == 80, "the complete 80-input corpus is required"
    assert len({sample["id"] for sample in samples}) == 80, "duplicate sample ID"
    paths = set()
    for sample in samples:
        assert len(sample["files"]) == 4, sample["id"]
        for source in sample["files"].values():
            relative = Path(source["path"])
            assert not relative.is_absolute() and ".." not in relative.parts
            assert relative not in paths, relative
            paths.add(relative)
            path = corpus / relative
            assert path.is_file(), f"required fixture missing: {path}"
            assert path.stat().st_size == source["size"], path
            assert sha(path) == source["sha256"], f"fixture hash changed: {path}"
    frozen = json.loads((corpus / "hypothesis_freeze.json").read_text(encoding="utf-8"))
    assert len(frozen["files"]) == 6
    for name, expected in frozen["files"].items():
        assert sha(root / name) == expected, f"frozen P0 rule changed: {name}"
    return {
        "samples": len(samples),
        "verified_files": len(paths),
        "verified_frozen_files": len(frozen["files"]),
        "manifest_sha256": sha(corpus / "manifest.json"),
        "freeze_sha256": sha(corpus / "hypothesis_freeze.json"),
        "native_outputs_regenerated": False,
        "third_party_samples_used": False,
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    result = check(args.root)
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print("JWC corpus: 80 inputs / 320 files and 6 frozen P0 files verified.")


if __name__ == "__main__":
    main()
