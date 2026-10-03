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
import zipfile
from email.parser import BytesParser
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(*args, **kwargs):
    return subprocess.run(args, check=True, encoding="utf-8", **kwargs)


def inspect_wheel(wheel, expected_version=None):
    with zipfile.ZipFile(wheel) as archive:
        names = archive.namelist()
        metadata_files = [n for n in names if n.endswith(".dist-info/METADATA")]
        assert len(metadata_files) == 1, "expected one wheel METADATA"
        metadata = BytesParser().parsebytes(archive.read(metadata_files[0]))
        assert metadata["Name"] == "ezjww"
        assert metadata["Requires-Python"] == ">=3.9"
        if expected_version:
            assert metadata["Version"] == expected_version, "wheel version mismatch"
        for filename in ("ezjww/py.typed", "ezjww/_core.pyi", "ezjww/__init__.py"):
            assert filename in names, "missing wheel API file: " + filename
        license_files = [n for n in names if n.endswith("/LICENSE")]
        assert license_files, "wheel must include LICENSE"
        assert any(
            archive.read(n) == (ROOT / "LICENSE").read_bytes() for n in license_files
        )
        assert all(n.startswith(("ezjww/", "ezjww-")) for n in names), (
            "unexpected wheel payload"
        )
        assert not any(
            ".internal" in Path(n).parts or "header_700.bin" in n for n in names
        )
    return {
        "version": metadata["Version"],
        "requires_python": metadata["Requires-Python"],
        "files": names,
    }


def probe(work):
    # This function runs from a copied script, using only the installed wheel.
    import ezjww
    from ezjww import _core
    from importlib.metadata import version

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
    empty = ezjww.readfile(work / "empty.jww")
    assert empty.header["version"] == 700
    assert len(empty.modelspace()) == 0 and empty.bbox() is None
    # New native creation must work with only the installed wheel and no template path.
    created = ezjww.new()
    assert created.to_jww_bytes() == (work / "empty.jww").read_bytes()
    msp = created.modelspace()
    msp.add_line((0, 0), (100, 0), jwwattribs={"pen_color": 3})
    msp.add_circle((20, 20), 5)
    msp.add_arc((40, 20), 5, 350, 30)
    msp.add_point((60, 20))
    msp.add_text("日本語𠮷", (0, -10), (12, -10))
    created.options["layer_groups"][0]["name"] = "平面図"
    generated = work / "created.jww"
    created.saveas(generated)
    assert generated.read_bytes().startswith(b"JwwData.")
    assert ezjww.read_document(str(generated)) == created.source_document
    created.save_dxf(work / "created.dxf", target_version="AC1024")
    assert (work / "created.dxf").read_text(encoding="utf-8") == created.to_dxf_string(
        target_version="AC1024"
    )
    assert created.stats()["entity_count"] == 5
    assert ezjww.new_dxf().header is None
    # Independently reconstruct the six-entity native-qualified acceptance case.
    basic = ezjww.new()
    modelspace = basic.modelspace()
    modelspace.add_line(
        (-50, -20),
        (50, -20),
        jwwattribs={"pen_style": 3, "pen_color": 3, "pen_width": 25, "layer": 2},
    )
    modelspace.add_circle((-30, 20), 10, jwwattribs={"pen_color": 2})
    modelspace.add_arc((0, 20), 10, 350, 30)
    modelspace.add_point((30, 20), jwwattribs={"pen_color": 4})
    modelspace.add_text(
        "日本語 ABC", (-50, -40), (-20, -40), spacing=0.5, jwwattribs={"pen_color": 5}
    )
    modelspace.add_text("縦方向", (30, -40), (30, -20), angle=90, size_y=5)
    assert basic.to_jww_bytes() == (work / "basic.jww").read_bytes()
    protected = work / "protected.dxf"
    protected.write_bytes(b"keep")
    try:
        basic.saveas(protected)
    except ValueError:
        pass
    else:
        raise AssertionError("legacy DXF saveas unexpectedly accepted")
    assert protected.read_bytes() == b"keep"
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
    flagged_path = str(work / "r011.jwc")
    flagged = ezjww.read_jwc_document(flagged_path)
    assert len(flagged["entities"]) == 1
    assert flagged["entities"][0]["attributes"]["flags_raw"] == 2
    assert len(flagged["diagnostics"]) == 1
    diagnostic = flagged["diagnostics"][0]
    assert diagnostic["code"] == "JWC_ATTRIBUTE_UNVERIFIED"
    assert diagnostic["severity"] == "warning" and diagnostic["action"] == "retained"
    assert diagnostic["details"] == {
        "field": "line.flags",
        "byte_offset": 2441,
        "count": 1,
        "values": ["0x0002"],
    }
    assert ezjww.read_cad_document(flagged_path) == {
        "format": "jwc",
        "document": flagged,
    }
    flagged_dxf = ezjww.read_dxf_document(flagged_path)
    assert len(flagged_dxf["entities"]) == 1
    assert flagged_dxf["entities"][0]["type"] == "LINE"
    assert flagged_dxf["jwc_conversion_report"]["diagnostics"] == flagged["diagnostics"]
    flagged_audit = json.loads(
        run(*base, "audit", flagged_path, "--json", capture_output=True).stdout
    )
    assert flagged_audit["has_issues"]
    assert flagged_audit["diagnostics"] == flagged["diagnostics"]
    flagged_output = work / "r011.dxf"
    run(*base, "to-dxf", flagged_path, "-o", str(flagged_output), capture_output=True)
    assert flagged_output.read_text(encoding="utf-8") == ezjww.read_dxf_string(
        flagged_path
    )

    for name, offset in [("r080", 2483)]:
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
        "version": version("ezjww"),
        "python": sys.version,
        "python_executable": sys.executable,
        "python_module": ezjww.__file__,
        "extension": _core.__file__,
        "extension_sha256": sha(Path(_core.__file__)),
        "console_script": str(console),
        "jwc_dxf_sha256": sha(cli_dxf),
        "writer_fixture_sha256": hashlib.sha256(basic.to_jww_bytes()).hexdigest(),
        "source_tree_imported": False,
        "checked": [
            "JWW",
            "empty version-700 JWW",
            "native JWW creation/save/readback and explicit DXF export",
            "exact native-qualified mixed geometry bytes and saveas migration",
            "Japanese JWC",
            "both coordinate spaces",
            "ellipse",
            "DXF AC1024",
            "CP932 diagnostics",
            "five JSON CLI readers",
            "DXF CLI",
            "unverified line flags and diagnostics (API and CLI)",
            "structural rejection",
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
    parser.add_argument("--expected-version")
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
        payload = inspect_wheel(wheel, args.expected_version)
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
        shutil.copyfile(ROOT / "jww_samples/writer/empty.jww", work / "empty.jww")
        shutil.copyfile(ROOT / "jww_samples/writer/basic/basic.jww", work / "basic.jww")
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
        assert result["version"] == payload["version"], "installed version mismatch"
        result.update(
            wheel=str(wheel),
            wheel_sha256=sha(wheel),
            workdir=str(work),
            payload=payload,
        )
    args.report.parent.mkdir(parents=True, exist_ok=True)
    args.report.write_text(
        json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    print("Installed wheel JWW/JWC API and CLI smoke passed.")


if __name__ == "__main__":
    main()
