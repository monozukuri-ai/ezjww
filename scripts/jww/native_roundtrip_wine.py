#!/usr/bin/env python3
"""Run Jw_cad 10.02.1 from Linux Python in a fresh Wine prefix and Xvfb display.

Requires wine, wineserver, Xvfb, xdotool and ImageMagick's import on PATH.
The output folder must not exist. Application files/fonts are supplied locally.
All UI operations are confined to the display/prefix created by this script.
"""

import argparse
import hashlib
import json
import os
import re
import shutil
import subprocess as sp
import time
from datetime import datetime, timezone
from pathlib import Path

EXE_SHA256 = "95e6b11c4ee014e0079f288429ae2c6e5eed41141a963e4b8770d2e8ead87acf"
FILES = (
    "Jw_win.exe",
    "JWW2DXFConv.dll",
    "common_lib.dll",
    "common_lib_AP202.dll",
    "DXF_HDR.DAT",
)


def digest(path):
    data = path.read_bytes()
    return {"size": len(data), "sha256": hashlib.sha256(data).hexdigest()}


def windows_path(path):
    return "Z:" + str(path.resolve()).replace("/", "\\")


class Session:
    def __init__(self, runtime, env, log):
        self.runtime, self.env, self.log = runtime, env, log
        self.process = None

    def run(self, *args, check=True, timeout=10):
        return sp.run(
            args,
            env=self.env,
            capture_output=True,
            text=True,
            check=check,
            timeout=timeout,
        )

    def xdo(self, *args, check=True):
        return self.run("xdotool", *map(str, args), check=check).stdout.strip()

    def window(self, title):
        result = self.xdo(
            "search",
            "--onlyvisible",
            "--name",
            "^" + re.escape(title) + "$",
            check=False,
        )
        return result.splitlines()[0] if result else None

    def wait(self, predicate, timeout=30):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            value = predicate()
            if value:
                return value
            time.sleep(0.2)
        raise RuntimeError(
            "UI timed out: "
            + self.xdo(
                "search",
                "--onlyvisible",
                "--name",
                ".",
                "getwindowname",
                "%@",
                check=False,
            )
        )

    def capture(self, path):
        self.run(
            "import", "-display", self.env["DISPLAY"], "-window", "root", str(path)
        )

    def launch(self, path):
        self.process = sp.Popen(
            ["wine", str(self.runtime / "Jw_win.exe"), windows_path(path)],
            cwd=self.runtime,
            env=self.env,
            stdout=self.log,
            stderr=self.log,
        )

        def ready():
            association = self.window("jw_win")
            if association:
                # A new prefix asks about file association. Cancel it; do not
                # install shell integration. Any other modal fails the title gate.
                # The dialog can disappear between search and windowfocus.
                # Continue polling for the drawing even if this action fails.
                self.xdo("windowfocus", association, "key", "Escape", check=False)
            return self.window(path.name + " - jw_win")

        self.main = self.wait(ready)
        self.xdo("windowmove", self.main, 0, 0, "windowsize", self.main, 1260, 860)
        time.sleep(1)

    def save(self, stem, extension):
        destination = self.runtime / f"{stem}.{extension}"
        if destination.exists():
            raise FileExistsError(destination)
        self.xdo("windowfocus", self.main, "key", "alt+f")
        time.sleep(0.5)  # Wine must paint the menu before its mnemonic is sent.
        self.xdo("key", "a" if extension == "jww" else "e")
        selector = self.wait(lambda: self.window("ファイル選択"))
        # Pinned 10.02.1 dialog: New is a non-mnemonic button at dialog units
        # (166, 0, 33, 14). Wine's default 96-DPI dialog maps its center here.
        # Both dialog titles and saved output are checked before continuing.
        geometry = dict(
            line.split("=", 1)
            for line in self.xdo("getwindowgeometry", "--shell", selector).splitlines()
        )
        self.xdo(
            "mousemove", int(geometry["X"]) + 315, int(geometry["Y"]) + 10, "click", 1
        )
        new = self.wait(lambda: self.window("新規作成"))
        self.xdo("windowfocus", new, "key", "ctrl+a")
        self.xdo("type", "--clearmodifiers", stem)
        self.xdo("key", "Return")
        self.wait(lambda: self.window(destination.name + " - jw_win"))
        self.wait(lambda: destination.is_file() and destination.stat().st_size > 0)
        before = digest(destination)
        time.sleep(1)
        if digest(destination) != before:
            raise RuntimeError("Output is still changing")
        return destination

    def close(self):
        self.xdo("windowfocus", self.main, "key", "alt+F4")
        code = self.process.wait(timeout=20)
        if code:
            raise RuntimeError(f"Jw_cad exited with {code}")
        self.process = None
        return code


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--install", type=Path, required=True)
    parser.add_argument("--inputs", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--display", default=":194")
    parser.add_argument(
        "--font-dir",
        type=Path,
        help="Optional directory of locally licensed .ttf/.ttc fonts",
    )
    parser.add_argument(
        "--cases",
        nargs="+",
        default=[
            "sxf_colors",
            "sxf_linetypes",
            "solid",
            "ellipse",
            "dimension",
            "block",
            "text_auto_end",
        ],
    )
    parser.add_argument("--jww-only", nargs="*", default=[])
    args = parser.parse_args()
    if not re.fullmatch(r":[0-9]+", args.display):
        parser.error("--display must be an X display number, e.g. :194")
    number = args.display[1:]
    if (
        Path(f"/tmp/.X{number}-lock").exists()
        or Path(f"/tmp/.X11-unix/X{number}").exists()
    ):
        raise RuntimeError("Display already exists; choose a free display")
    if len(set(args.cases)) != len(args.cases) or any(
        not re.fullmatch(r"[a-z0-9_]+", s) for s in args.cases
    ):
        parser.error("case names must be unique lowercase ASCII basenames")
    if not set(args.jww_only).issubset(args.cases):
        parser.error("--jww-only must name selected cases")
    if digest(args.install / "Jw_win.exe")["sha256"] != EXE_SHA256:
        raise ValueError("unverified Jw_cad executable")
    for command in ("wine", "wineserver", "Xvfb", "xdotool", "import"):
        if shutil.which(command) is None:
            raise RuntimeError(f"missing command: {command}")
    args.output.mkdir(parents=True, exist_ok=False)
    output = args.output.resolve()
    # Capture the exact runner before launching processes, even if the working
    # checkout is later edited while a long native run is in progress.
    runner = output / "native_roundtrip_wine.py"
    shutil.copyfile(__file__, runner)
    runtime, prefix = output / "runtime", output / "prefix"
    runtime.mkdir()
    prefix.mkdir()
    for name in FILES:
        shutil.copy2(args.install / name, runtime / name)
    for name in args.cases:
        shutil.copy2(args.inputs / f"{name}.jww", runtime / f"{name}.jww")
    env = dict(
        os.environ, DISPLAY=args.display, WINEPREFIX=str(prefix), WINEDEBUG="-all"
    )
    report = dict(
        started_at_utc=datetime.now(timezone.utc).isoformat(),
        script_sha256=digest(runner)["sha256"],
        environment={
            "kind": "wine-linux-python",
            "wine": sp.check_output(["wine", "--version"], text=True).strip(),
        },
        application={name: digest(runtime / name) for name in FILES},
        cases=[],
        complete=False,
    )
    session = None
    with (output / "process.log").open("w") as log:
        server = sp.Popen(
            [
                "Xvfb",
                args.display,
                "-screen",
                "0",
                "1280x960x24",
                "-nolisten",
                "tcp",
                "-noreset",
            ],
            stdout=log,
            stderr=log,
        )
        try:
            time.sleep(1)
            if server.poll() is not None:
                raise RuntimeError("Xvfb failed")
            sp.run(
                ["wineboot", "-u"],
                env=env,
                stdout=log,
                stderr=log,
                timeout=60,
                check=True,
            )
            if args.font_dir:
                for font in args.font_dir.iterdir():
                    if font.suffix.lower() in (".ttf", ".ttc"):
                        shutil.copy2(font, prefix / "drive_c/windows/Fonts" / font.name)
            for name in ("File", "FileC"):
                sp.run(
                    [
                        "wine",
                        "reg",
                        "add",
                        r"HKCU\Software\Jw_cad\jw_win\Folder",
                        "/v",
                        name,
                        "/t",
                        "REG_SZ",
                        "/d",
                        windows_path(runtime),
                        "/f",
                    ],
                    env=env,
                    stdout=log,
                    stderr=log,
                    timeout=20,
                    check=True,
                )
            session = Session(runtime, env, log)
            for name in args.cases:
                print(name, flush=True)
                original = runtime / f"{name}.jww"
                before = digest(original)
                event = {"id": name, "complete": False}
                report["cases"].append(event)
                session.launch(original)
                session.capture(runtime / f"{name}_input.png")
                saved = session.save(name + "_saved", "jww")
                event["first_exit_code"] = session.close()
                session.launch(saved)
                reopened = session.save(name + "_reopened", "jww")
                session.capture(runtime / f"{name}_reopened.png")
                event["second_exit_code"] = session.close()
                if name not in args.jww_only:
                    # Launch afresh: Wine can swallow menu input immediately
                    # after saving/rebuilding the native toolbar.
                    session.launch(reopened)
                    session.save(name + "_reopened", "dxf")
                    event["dxf_exit_code"] = session.close()
                # A derived GDI copy isolates Wine's Direct2D display behavior.
                data = reopened.read_bytes()
                old, new = (f"View_Direct2d = {v}".encode("utf-16-le") for v in (1, 0))
                if data.count(old) == 1:
                    gdi = runtime / f"{name}_gdi.jww"
                    with gdi.open("xb") as f:
                        f.write(data.replace(old, new))
                    session.launch(gdi)
                    session.capture(runtime / f"{name}_gdi.png")
                    event["gdi_exit_code"] = session.close()
                event["input_unchanged"] = digest(original) == before
                if not event["input_unchanged"]:
                    raise RuntimeError("Original input changed")
                event["files"] = {
                    p.name: digest(p) for p in sorted(runtime.glob(name + "*"))
                }
                event["complete"] = True
            report["complete"] = True
        except BaseException as error:
            report["error"] = str(error)
            if session:
                try:
                    session.capture(output / "failure.png")
                except Exception:
                    pass
            raise
        finally:
            # Only this newly created prefix and X server are terminated.
            sp.run(["wineserver", "-k"], env=env, stdout=log, stderr=log, timeout=15)
            server.terminate()
            server.wait(timeout=10)
            report["finished_at_utc"] = datetime.now(timezone.utc).isoformat()
            (output / "native.json").write_text(
                json.dumps(report, indent=2) + "\n", encoding="utf-8"
            )


if __name__ == "__main__":
    main()
