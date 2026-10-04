"""Reject distribution payload damage and publishing an unqualified artifact."""

import hashlib
import io
import json
import runpy
import sys
import tarfile
from pathlib import Path

import pytest

ROOT = Path(__file__).resolve().parents[2]
SDIST = runpy.run_path(str(ROOT / "scripts/check-sdist.py"))


def source_archive(tmp_path, mutation):
    files = {name: b"placeholder" for name in SDIST["REQUIRED"]}
    files["PKG-INFO"] = b"Name: ezjww\nVersion: 0.5.0\nRequires-Python: >=3.9\n\n"
    files[SDIST["TEMPLATE"]] = (ROOT / SDIST["TEMPLATE"]).read_bytes()
    if mutation == "missing":
        files.pop(SDIST["TEMPLATE"])
    elif mutation == "workspace":
        files.pop("crates/ezjww-wasm/Cargo.toml")
    elif mutation == "changed":
        files[SDIST["TEMPLATE"]] = b"broken"
    elif mutation == "binary":
        files["src/ezjww/_core.so"] = b"build residue"
    elif mutation == "escape":
        files["../../escape"] = b"unsafe"
    path = tmp_path / "source.tar.gz"
    with tarfile.open(path, "w:gz") as archive:
        for name, data in files.items():
            member = tarfile.TarInfo("ezjww-0.5.0/" + name)
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
    return path


@pytest.mark.parametrize(
    "mutation", ["missing", "workspace", "changed", "binary", "escape"]
)
def test_sdist_rejects_missing_template_corruption_and_build_residue(
    tmp_path, mutation
):
    with tarfile.open(source_archive(tmp_path, mutation)) as archive:
        with pytest.raises(ValueError):
            SDIST["inspect_sdist"](archive, "0.5.0")


def test_sdist_metadata_and_template(tmp_path):
    with tarfile.open(source_archive(tmp_path, None)) as archive:
        assert (
            SDIST["inspect_sdist"](archive, "0.5.0")["template_sha256"]
            == SDIST["TEMPLATE_SHA256"]
        )
        with pytest.raises(ValueError, match="version mismatch"):
            SDIST["inspect_sdist"](archive, "0.3.4")


@pytest.mark.skipif(
    sys.version_info < (3, 11), reason="release tooling uses Python 3.11+"
)
def test_publish_gate_requires_matching_versions_and_every_qualified_hash(tmp_path):
    release = runpy.run_path(str(ROOT / "scripts/check-release.py"))
    assert release["check_versions"](tag="v0.5.0") == "0.5.0"
    with pytest.raises(ValueError, match="tag"):
        release["check_versions"](tag="v0.3.4")
    names = ["linux.whl", "macos.whl", "windows.whl", "source.tar.gz", "package.tgz"]
    for index, name in enumerate(names):
        data = name.encode()
        (tmp_path / name).write_bytes(data)
        key = "wheel" if name.endswith(".whl") else "archive"
        report = {
            "version": "0.5.0",
            key: "C:\\build\\" + name,
            key + "_sha256": hashlib.sha256(data).hexdigest(),
        }
        (tmp_path / f"{index}-verification.json").write_text(json.dumps(report))
    assert release["verify_artifacts"](tmp_path, "0.5.0") == sorted(names)
    (tmp_path / "linux.whl").write_bytes(b"different build")
    with pytest.raises(ValueError, match="artifact changed"):
        release["verify_artifacts"](tmp_path, "0.5.0")
    (tmp_path / "linux.whl").write_bytes(b"linux.whl")
    (tmp_path / "0-verification.json").unlink()
    with pytest.raises(ValueError, match="missing artifact"):
        release["verify_artifacts"](tmp_path, "0.5.0")
