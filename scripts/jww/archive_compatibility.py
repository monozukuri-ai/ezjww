#!/usr/bin/env python3
"""Archive original native run artifacts and measured expectations in a new folder.

Runs are captured by native_roundtrip.py in Windows/Wine. This step records the
oracle's outputs; check_compatibility.py independently checks input preservation
and native DXF geometry. It does not declare every native change lossless.
"""

from __future__ import annotations

import argparse
import gzip
import json
from pathlib import Path

from check_compatibility import (
    CASES,
    EXE_SHA256,
    DXF_HEADER_SHA256,
    check,
    digest,
    is_setting,
    read_dxf_tags,
)


def main():
    import ezjww

    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--run",
        nargs=3,
        action="append",
        required=True,
        metavar=("LOG", "RUNTIME", "SCRIPT"),
    )
    parser.add_argument("--writer-revision", required=True)
    parser.add_argument(
        "--display-run", nargs=3, required=True, metavar=("LOG", "RUNTIME", "SCRIPT")
    )
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output
    output.mkdir()  # Refuse replacement of an existing evidence directory.
    manifest = dict(
        schema_version=1,
        license="Original drawings and tools: repository MIT; screenshots include Jw_cad UI.",
        application_version="10.02.1",
        executable_sha256=EXE_SHA256,
        writer_base_revision=args.writer_revision,
        dxf_header_sha256=DXF_HEADER_SHA256,
        windows_validation="not_run",
        artifacts={},
        runs=[],
        run_scripts={},
        cases=[],
    )
    cases = {}

    def store(name, data):
        if Path(name).name != name or "\\" in name:
            raise ValueError("artifact names must be basenames")
        if name in manifest["artifacts"]:
            if manifest["artifacts"][name]["original"] != digest(data):
                raise ValueError("different artifacts have the same name: " + name)
            return
        # Keep exact native bytes; gzip bounds corpus checkout size without
        # changing the native source file names recorded in the run logs.
        packed = gzip.compress(data, mtime=0)
        path = name + ".gz"
        (output / path).write_bytes(packed)
        manifest["artifacts"][name] = dict(
            path=path, original=digest(data), stored=digest(packed)
        )

    for index, (log_path, runtime_path, script_path) in enumerate(args.run):
        log, runtime = Path(log_path), Path(runtime_path)
        for filename, expected in [
            ("Jw_win.exe", EXE_SHA256),
            ("DXF_HDR.DAT", DXF_HEADER_SHA256),
        ]:
            if digest((runtime / filename).read_bytes())["sha256"] != expected:
                raise ValueError("unverified native runtime file: " + filename)
        run = json.loads(log.read_text(encoding="utf-8"))
        run_name = f"native-run-{index + 1}.json"
        script_name = f"native-run-{index + 1}.py"
        store(run_name, log.read_bytes())
        store(script_name, Path(script_path).read_bytes())
        manifest["runs"].append(run_name)
        manifest["run_scripts"][run_name] = script_name
        for event in run["cases"]:
            name = event["id"]
            if name in cases:
                raise ValueError("duplicate case: " + name)
            for filename, expected in event["files"].items():
                data = (runtime / filename).read_bytes()
                if digest(data) != expected:
                    raise ValueError("native artifact changed: " + filename)
                store(filename, data)
            source = ezjww.read_document(str(runtime / (name + ".jww")))
            saved = ezjww.read_document(str(runtime / (name + "_reopened.jww")))
            texts = [
                e
                for e in saved["entities"]
                if e["type"] == "TEXT" and not is_setting(e)
            ]
            has_dxf = event["status"] == "save_reopen_save_export"
            case = dict(
                id=name,
                input_entities=len(source["entities"]),
                native_dxf=has_dxf,
                native_memo=saved["header"]["memo"],
                native_text_endpoints=[[e["end_x"], e["end_y"]] for e in texts],
            )
            if has_dxf:
                tags = read_dxf_tags((runtime / (name + "_reopened.dxf")).read_bytes())
                case["native_dxf_attributes"] = [
                    {str(k): e.get(k) for k in (8, 6, 62)} for e in tags
                ]
                # These are measured settings of this pinned template/app.
                case["native_dxf_transform"] = (
                    [0.5, 148.5, 105.0] if name == "settings" else [1.0, 210.0, 148.5]
                )
            cases[name] = case
    if set(cases) != set(CASES):
        raise ValueError("incomplete matrix")
    manifest["cases"] = [cases[name] for name in CASES]
    log_path, runtime_path, script_path = map(Path, args.display_run)
    display = json.loads(log_path.read_text(encoding="utf-8"))
    manifest["display_run"] = "native-display.json"
    manifest["run_scripts"]["native-display.json"] = "native-display.py"
    store("native-display.json", log_path.read_bytes())
    store("native-display.py", script_path.read_bytes())
    for event in display["cases"]:
        for filename, expected in event["files"].items():
            data = (runtime_path / filename).read_bytes()
            if digest(data) != expected:
                raise ValueError("display artifact changed: " + filename)
            store(filename, data)
    (output / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8"
    )
    result = check(output)
    (output / "validation.json").write_text(
        json.dumps(result, indent=2) + "\n", encoding="utf-8"
    )
    print(f"Archived and checked {len(cases)} native cases in {output}")


if __name__ == "__main__":
    main()
