"""Exercise Wine launch races without requiring a display or Jw_cad."""

import itertools
import runpy
import subprocess
from pathlib import Path
from types import SimpleNamespace

import pytest

SCRIPT = Path(__file__).resolve().parents[2] / "scripts/jww/native_roundtrip_wine.py"


def mock_session(
    tmp_path, monkeypatch, *, dialog_status=1, main_visible=True, resize_status=0
):
    session_class = runpy.run_path(str(SCRIPT))["Session"]
    commands = []
    association_polls = 0
    drawing_polls = 0

    def run(args, **kwargs):
        nonlocal association_polls, drawing_polls
        commands.append(args)
        status, output = 0, ""
        if args[1] == "search":
            if args[-1] == "^jw_win$":
                association_polls += 1
                output = "101" if association_polls == 1 else ""
            else:
                drawing_polls += 1
                # The drawing becomes visible on a later poll after the dialog.
                output = "202" if main_visible and drawing_polls > 1 else ""
            status = 0 if output else 1
        elif args[1] == "windowfocus":
            assert args == ("xdotool", "windowfocus", "101", "key", "Escape")
            status = dialog_status
        elif args[1] == "windowmove":
            status = resize_status
        else:
            raise AssertionError(args)
        result = subprocess.CompletedProcess(args, status, stdout=output, stderr="")
        if kwargs["check"]:
            result.check_returncode()
        return result

    # Replace only the loaded script's dependencies; never start native programs.
    globals_ = session_class.launch.__globals__
    monkeypatch.setitem(
        globals_, "sp", SimpleNamespace(run=run, Popen=lambda *args, **kwargs: object())
    )
    clock = itertools.count()
    monkeypatch.setitem(
        globals_,
        "time",
        SimpleNamespace(monotonic=lambda: next(clock), sleep=lambda _: None),
    )
    return session_class(tmp_path, {"DISPLAY": ":194"}, None), commands


@pytest.mark.parametrize("filename", ["block.jww", "block_gdi.jww"])
@pytest.mark.parametrize("dialog_status", [0, 1])
def test_launch_accepts_closed_or_vanished_association_dialog(
    tmp_path, monkeypatch, filename, dialog_status
):
    session, commands = mock_session(tmp_path, monkeypatch, dialog_status=dialog_status)
    session.launch(tmp_path / filename)
    assert session.main == "202"
    assert commands[-1] == (
        "xdotool",
        "windowmove",
        "202",
        "0",
        "0",
        "windowsize",
        "202",
        "1260",
        "860",
    )


def test_launch_still_requires_drawing_window(tmp_path, monkeypatch):
    session, commands = mock_session(tmp_path, monkeypatch, main_visible=False)
    with pytest.raises(RuntimeError, match="UI timed out"):
        session.launch(tmp_path / "block.jww")
    assert not any(args[1] == "windowmove" for args in commands)


def test_launch_still_rejects_main_window_command_failure(tmp_path, monkeypatch):
    session, _ = mock_session(tmp_path, monkeypatch, resize_status=1)
    with pytest.raises(subprocess.CalledProcessError) as error:
        session.launch(tmp_path / "block_gdi.jww")
    assert error.value.cmd[1] == "windowmove"
