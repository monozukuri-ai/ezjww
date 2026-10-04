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


@pytest.fixture(scope="module")
def project_version() -> str:
    # JSON works on Python 3.9 without an extra TOML dependency. The release
    # gate independently checks this version against Python, Rust and lockfiles.
    manifest = ROOT / "packages/ezjww/package.json"
    return json.loads(manifest.read_text(encoding="utf-8"))["version"]


@pytest.fixture(scope="module")
def mismatched_version(project_version: str) -> str:
    prefix, _, patch = project_version.rpartition(".")
    return f"{prefix}.{int(patch) + 1}"


def source_archive(tmp_path, mutation, version):
    files = {name: b"placeholder" for name in SDIST["REQUIRED"]}
    files["PKG-INFO"] = (
        f"Name: ezjww\nVersion: {version}\nRequires-Python: >=3.9\n\n"
    ).encode()
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
            member = tarfile.TarInfo(f"ezjww-{version}/" + name)
            member.size = len(data)
            archive.addfile(member, io.BytesIO(data))
    return path


@pytest.mark.parametrize(
    "mutation", ["missing", "workspace", "changed", "binary", "escape"]
)
def test_sdist_rejects_missing_template_corruption_and_build_residue(
    tmp_path, mutation, project_version
):
    path = source_archive(tmp_path, mutation, project_version)
    with tarfile.open(path) as archive, pytest.raises(ValueError):
        SDIST["inspect_sdist"](archive, project_version)


def test_sdist_metadata_and_template(tmp_path, project_version, mismatched_version):
    with tarfile.open(source_archive(tmp_path, None, project_version)) as archive:
        assert (
            SDIST["inspect_sdist"](archive, project_version)["template_sha256"]
            == SDIST["TEMPLATE_SHA256"]
        )
        with pytest.raises(ValueError, match="version mismatch"):
            SDIST["inspect_sdist"](archive, mismatched_version)


@pytest.mark.skipif(
    sys.version_info < (3, 11), reason="release tooling uses Python 3.11+"
)
def test_publish_gate_requires_matching_versions_and_every_qualified_hash(
    tmp_path, project_version, mismatched_version
):
    release = runpy.run_path(str(ROOT / "scripts/check-release.py"))
    assert release["check_versions"](tag=f"v{project_version}") == project_version
    with pytest.raises(ValueError, match="tag"):
        release["check_versions"](tag=f"v{mismatched_version}")
    names = ["linux.whl", "macos.whl", "windows.whl", "source.tar.gz", "package.tgz"]
    for index, name in enumerate(names):
        data = name.encode()
        (tmp_path / name).write_bytes(data)
        key = "wheel" if name.endswith(".whl") else "archive"
        report = {
            "version": project_version,
            key: "C:\\build\\" + name,
            key + "_sha256": hashlib.sha256(data).hexdigest(),
        }
        (tmp_path / f"{index}-verification.json").write_text(json.dumps(report))
    assert release["verify_artifacts"](tmp_path, project_version) == sorted(names)
    (tmp_path / "linux.whl").write_bytes(b"different build")
    with pytest.raises(ValueError, match="artifact changed"):
        release["verify_artifacts"](tmp_path, project_version)
    (tmp_path / "linux.whl").write_bytes(b"linux.whl")
    (tmp_path / "0-verification.json").unlink()
    with pytest.raises(ValueError, match="missing artifact"):
        release["verify_artifacts"](tmp_path, project_version)
