#!/usr/bin/env python3
"""Build an extracted sdist outside the checkout, then verify its installed wheel.

Requires Rust, pip and maturin in the caller's Python environment. Build tools
and Cargo dependencies must be available; the ezjww source comes only from the
archive. The runtime probe uses a second, dependency-free virtual environment.
"""

import argparse
import hashlib
import json
import os
import subprocess
import sys
import tarfile
import tempfile
from email.parser import BytesParser
from pathlib import Path, PurePosixPath

TEMPLATE = "crates/ezjww-core/src/writer/templates/header_700.bin"
TEMPLATE_SHA256 = "7d66238bef9a66e3030a39a58936ba6aedc2189a3ebe46d582c908bf43622572"
REQUIRED = {
    "PKG-INFO",
    "LICENSE",
    "README.md",
    "pyproject.toml",
    "Cargo.toml",
    "Cargo.lock",
    "src/lib.rs",
    "src/ezjww/__init__.py",
    "src/ezjww/_core.pyi",
    "src/ezjww/py.typed",
    "crates/ezjww-core/Cargo.toml",
    "crates/ezjww-core/src/writer/mod.rs",
    "crates/ezjww-wasm/Cargo.toml",
    "crates/ezjww-wasm/src/lib.rs",
    TEMPLATE,
    "scripts/check-wheel.py",
    "jww_samples/writer/basic/basic.jww",
}


def inspect_sdist(archive, expected_version=None):
    members = archive.getmembers()
    names = [m.name for m in members]
    if not names or len(set(names)) != len(names):
        raise ValueError("empty archive or duplicate members")
    roots = set()
    for member in members:
        path = PurePosixPath(member.name)
        if (
            path.is_absolute()
            or ".." in path.parts
            or "\\" in member.name
            or not (member.isfile() or member.isdir())
        ):
            raise ValueError("unsafe sdist member: " + member.name)
        roots.add(path.parts[0])
        if set(path.parts) & {
            ".internal",
            ".git",
            ".venv",
            "node_modules",
            "target",
            "__pycache__",
        } or path.suffix in {".so", ".pyd", ".dll", ".exe", ".whl", ".pyc"}:
            raise ValueError("unexpected sdist payload: " + member.name)
    if len(roots) != 1:
        raise ValueError("sdist must have one root directory")
    root = roots.pop()
    files = {m.name[len(root) + 1 :] for m in members if m.isfile()}
    missing = REQUIRED - files
    if missing:
        raise ValueError("sdist missing: " + ", ".join(sorted(missing)))
    metadata = BytesParser().parsebytes(archive.extractfile(root + "/PKG-INFO").read())
    version = metadata["Version"]
    if (
        metadata["Name"] != "ezjww"
        or metadata["Requires-Python"] != ">=3.9"
        or root != "ezjww-" + version
    ):
        raise ValueError("unexpected sdist metadata")
    if expected_version and version != expected_version:
        raise ValueError("sdist version mismatch")
    template = archive.extractfile(root + "/" + TEMPLATE).read()
    if hashlib.sha256(template).hexdigest() != TEMPLATE_SHA256:
        raise ValueError("sdist writer template changed")
    return dict(
        root=root,
        version=version,
        file_count=len(files),
        template_sha256=TEMPLATE_SHA256,
    )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--archive", type=Path, required=True)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--expected-version")
    args = parser.parse_args()
    archive_path = args.archive.resolve(strict=True)
    work = Path(tempfile.mkdtemp(prefix="ezjww-sdist-"))
    with tarfile.open(archive_path, "r:gz") as archive:
        payload = inspect_sdist(archive, args.expected_version)
        # Only validated ordinary files/directories are extracted; no links or
        # absolute/parent paths. Works with the supported Python 3.9+ baseline.
        filters = {"filter": "data"} if hasattr(tarfile, "data_filter") else {}
        archive.extractall(work, **filters)
    source = work / payload["root"]
    lock = (source / "Cargo.lock").read_bytes()
    env = dict(
        os.environ,
        CARGO_TARGET_DIR=str(work / "target"),
        PYTHONUTF8="1",
        PIP_NO_CACHE_DIR="1",
        PIP_DISABLE_PIP_VERSION_CHECK="1",
    )
    for key in ("PYTHONPATH", "PYTHONHOME", "PYO3_PYTHON", "VIRTUAL_ENV"):
        env.pop(key, None)
    wheels = work / "wheels"
    subprocess.run(
        [
            sys.executable,
            "-I",
            "-m",
            "pip",
            "wheel",
            "--no-deps",
            "--no-build-isolation",
            "--config-settings=build-args=--locked",
            "--wheel-dir",
            str(wheels),
            str(source),
        ],
        cwd=work,
        env=env,
        check=True,
    )
    if (source / "Cargo.lock").read_bytes() != lock:
        raise ValueError("sdist build rewrote Cargo.lock")
    wheel_report = work / "wheel.json"
    subprocess.run(
        [
            sys.executable,
            "-I",
            str(source / "scripts/check-wheel.py"),
            "--wheel-dir",
            str(wheels),
            "--expected-version",
            payload["version"],
            "--report",
            str(wheel_report),
        ],
        cwd=work,
        env=env,
        check=True,
    )
    result = dict(
        version=payload["version"],
        cargo_lock_sha256=hashlib.sha256(lock).hexdigest(),
        archive=str(archive_path),
        archive_sha256=hashlib.sha256(archive_path.read_bytes()).hexdigest(),
        workdir=str(work),
        payload=payload,
        wheel=json.loads(wheel_report.read_text(encoding="utf-8")),
        source_checkout_used_for_build=False,
    )
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print("Source distribution build and installed JWW/JWC checks passed.")


if __name__ == "__main__":
    main()
