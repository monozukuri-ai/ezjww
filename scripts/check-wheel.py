#!/usr/bin/env python3
"""Install a wheel in a fresh external venv and check its JWW/JWC API and CLI."""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args, **kwargs):
    return subprocess.run(args, check=True, encoding="utf-8", **kwargs)


def probe(work):
    # This function runs from a copied script, using only the installed wheel.
    import ezjww
    from ezjww import _core

    prefix = Path(sys.prefix).resolve()
    for module in (ezjww, _core):
        Path(module.__file__).resolve().relative_to(prefix)
    package = Path(ezjww.__file__).parent
    assert (package / "py.typed").is_file() and (package / "_core.pyi").is_file()
    source = str(work / "q032.jwc")
    jww = str(work / "Test1.jww")
    cad = ezjww.read_cad_document(source)
    assert cad["format"] == "jwc"
    assert cad["document"]["header"]["source_version"] is None
    assert cad["document"]["entities"][0]["content"] == "日本語"
    assert ezjww.read_document(jww)["header"]["version"] == 600
    assert ezjww.read_cad_document(jww)["format"] == "jww"
    assert ezjww.read_dxf_string(jww).lstrip().startswith("0\nSECTION\n")
    paper = ezjww.readfile(work / "q054.jwc")
    model = ezjww.readfile(work / "q054.jwc", jwc_coordinates="model_millimeters")
    assert abs(model.bbox()["width"] / paper.bbox()["width"] - 50) < 1e-10
    assert (
        ezjww.read_dxf_document(str(work / "r013.jwc"))["entities"][0]["type"]
        == "ELLIPSE"
    )
    dxf_path = work / "text.dxf"
    report = ezjww.write_dxf_with_report(source, str(dxf_path), target_version="AC1024")
    assert report["source_version"] is None and report["source_format"] == "jwc"
    assert dxf_path.read_text(encoding="utf-8") == ezjww.read_dxf_string(
        source, target_version="AC1024"
    )
    tags = [line.strip() for line in dxf_path.read_text(encoding="utf-8").splitlines()]
    index = tags.index("$INSUNITS")
    assert tags[index + 1 : index + 3] == ["70", "4"]

    # A corrupt CP932 sequence is reported, while the source bytes are retained.
    data = bytearray((work / "q030.jwc").read_bytes())
    text = ezjww.read_jwc_document(str(work / "q030.jwc"))["entities"][0]
    span = text["string_source"]
    data[span["byte_offset"] + span["byte_length"] - 2] = 0x81
    replacement = work / "replacement.jwc"
    replacement.write_bytes(data)
    audit = ezjww.audit(replacement)
    assert audit["decode_error_count"] == 1 and audit["has_issues"]
    assert audit["diagnostics"][0]["code"] == "CP932_DECODE_REPLACED"

    base = [sys.executable, "-I", "-X", "utf8", "-m", "ezjww"]
    for command in ["info", "audit", "bbox", "stats", "report"]:
        payload = json.loads(
            run(*base, command, source, "--json", capture_output=True).stdout
        )
        assert payload
    cli_dxf = work / "cli.dxf"
    run(*base, "to-dxf", source, "-o", str(cli_dxf), capture_output=True)
    assert cli_dxf.read_text(encoding="utf-8") == ezjww.read_dxf_string(source)
    for name, offset in [("r011", 2441), ("r080", 2483)]:
        path = str(work / f"{name}.jwc")
        for read in [
            ezjww.read_jwc_document,
            ezjww.read_cad_document,
            ezjww.read_dxf_string,
        ]:
            try:
                read(path)
            except ValueError as error:
                assert f"byte {offset}" in str(error)
            else:
                raise AssertionError(f"unsupported input accepted: {name}")
        output = work / f"{name}.dxf"
        result = subprocess.run(
            base + ["to-dxf", path, "-o", str(output)],
            check=False,
            capture_output=True,
            encoding="utf-8",
        )
        assert result.returncode == 2 and f"byte {offset}" in result.stderr
        assert not output.exists()
    inputs = work / "mixed"
    inputs.mkdir()
    shutil.copyfile(source, inputs / "a.JwC")
    shutil.copyfile(jww, inputs / "b.JWW")
    run(
        *base,
        "to-dxf-dir",
        str(inputs),
        "-o",
        str(work / "mixed-dxf"),
        capture_output=True,
    )
    assert len(list((work / "mixed-dxf").glob("*.dxf"))) == 2
    shutil.copyfile(jww, inputs / "a.jww")
    result = subprocess.run(
        base + ["to-dxf-dir", str(inputs), "-o", str(work / "collision")],
        check=False,
        capture_output=True,
        encoding="utf-8",
    )
    assert result.returncode == 2 and "output collision" in result.stderr
    assert not (work / "collision").exists()
    console = prefix / ("Scripts/ezjww.exe" if os.name == "nt" else "bin/ezjww")
    result = run(str(console), "info", source, "--json", capture_output=True)
    assert json.loads(result.stdout)["source_format"] == "jwc"
    return {
        "python": sys.version,
        "python_executable": sys.executable,
        "python_module": ezjww.__file__,
        "extension": _core.__file__,
        "extension_sha256": sha(Path(_core.__file__)),
        "console_script": str(console),
        "jwc_dxf_sha256": sha(cli_dxf),
        "source_tree_imported": False,
        "checked": [
            "JWW",
            "Japanese JWC",
            "both coordinate spaces",
            "ellipse",
            "DXF AC1024",
            "CP932 diagnostics",
            "five JSON CLI readers",
            "DXF CLI",
            "two rejections",
            "mixed batch",
            "collision before write",
            "console script",
        ],
    }


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    wheels = parser.add_mutually_exclusive_group()
    wheels.add_argument("--wheel", type=Path)
    wheels.add_argument("--wheel-dir", type=Path)
    parser.add_argument("--report", type=Path, required=True)
    parser.add_argument("--probe", type=Path, help=argparse.SUPPRESS)
    args = parser.parse_args()
    if args.probe:
        result = probe(args.probe)
    else:
        if args.wheel_dir:
            candidates = list(args.wheel_dir.glob("*.whl"))
            assert len(candidates) == 1, (
                f"expected exactly one wheel in {args.wheel_dir}"
            )
            args.wheel = candidates[0]
        if args.wheel is None:
            parser.error("--wheel or --wheel-dir is required")
        wheel = args.wheel.resolve(strict=True)
        work = Path(tempfile.mkdtemp(prefix="ezjww-wheel-"))
        assert ROOT not in work.parents, (
            "wheel verification must run outside the source tree"
        )
        venv = work / "venv"
        run(sys.executable, "-m", "venv", str(venv))
        python = venv / ("Scripts/python.exe" if os.name == "nt" else "bin/python")
        env = dict(
            os.environ,
            PYTHONUTF8="1",
            PIP_DISABLE_PIP_VERSION_CHECK="1",
            PIP_NO_CACHE_DIR="1",
        )
        env.pop("PYTHONPATH", None)
        env.pop("PYTHONHOME", None)
        run(
            str(python),
            "-I",
            "-m",
            "pip",
            "install",
            "--no-index",
            "--no-deps",
            str(wheel),
            cwd=work,
            env=env,
        )
        for name in ["q030", "q032", "q054", "r013", "r011", "r080"]:
            shutil.copyfile(
                ROOT / "jwc_samples/generated" / f"{name}.jwc", work / f"{name}.jwc"
            )
        shutil.copyfile(ROOT / "jww_samples/Test1.jww", work / "Test1.jww")
        script = work / "probe.py"
        shutil.copyfile(__file__, script)
        local_report = work / "validation.json"
        run(
            str(python),
            "-I",
            "-X",
            "utf8",
            str(script),
            "--probe",
            str(work),
            "--report",
            str(local_report),
            cwd=work,
            env=env,
        )
        result = json.loads(local_report.read_text(encoding="utf-8"))
        result.update(wheel=str(wheel), wheel_sha256=sha(wheel), workdir=str(work))
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print("Installed wheel JWW/JWC API and CLI smoke passed.")


if __name__ == "__main__":
    main()
