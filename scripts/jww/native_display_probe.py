#!/usr/bin/env python3
"""Compare Wine display with Direct2D disabled in copies of native-saved files.

This diagnostic is not a writer API. It changes one known internal setting in
the recorded circle/basic native outputs and never modifies the originals.
"""

import argparse
import json
from datetime import datetime, timezone
from pathlib import Path

from native_roundtrip import (
    EXE_SHA256,
    NativeSession,
    digest,
    environment,
    output_folders,
)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--runtime", type=Path, required=True)
    parser.add_argument("--log", type=Path, required=True)
    args = parser.parse_args()
    runtime = args.runtime.resolve()
    if args.log.exists():
        raise FileExistsError(args.log)
    if digest(runtime / "Jw_win.exe")["sha256"] != EXE_SHA256:
        raise ValueError("unverified executable")
    old = "View_Direct2d = 1".encode("utf-16-le")
    new = "View_Direct2d = 0".encode("utf-16-le")
    for name in ("circle", "basic"):
        if (runtime / (name + "_reopened.jww")).read_bytes().count(old) != 1:
            raise ValueError("expected exactly one native Direct2D setting")
        for suffix in ("_gdi.jww", "_gdi.bmp"):
            if (runtime / (name + suffix)).exists():
                raise FileExistsError(name + suffix)
    report = dict(
        executable_sha256=EXE_SHA256,
        environment=environment(),
        script_sha256=digest(Path(__file__))["sha256"],
        started_at_utc=datetime.now(timezone.utc).isoformat(),
        cases=[],
    )
    session = NativeSession(runtime)
    try:
        with output_folders(runtime):
            for name in ("circle", "basic"):
                source = runtime / (name + "_reopened.jww")
                derived = runtime / (name + "_gdi.jww")
                screenshot = runtime / (name + "_gdi.bmp")
                before = digest(source)
                with derived.open("xb") as output:
                    output.write(source.read_bytes().replace(old, new))
                session.launch(derived)
                session.screenshot(screenshot)
                code = session.close()
                if digest(source) != before:
                    raise RuntimeError("source changed")
                report["cases"].append(
                    dict(
                        id=name,
                        exit_code=code,
                        input_unchanged=True,
                        files={
                            p.name: digest(p) for p in (source, derived, screenshot)
                        },
                    )
                )
    finally:
        try:
            if session.process is not None and session.process.poll() is None:
                session.process.terminate()
                session.process.wait(timeout=15)
        finally:
            report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
            with args.log.open("x", encoding="utf-8") as output:
                json.dump(report, output, indent=2)
                output.write("\n")


if __name__ == "__main__":
    main()
