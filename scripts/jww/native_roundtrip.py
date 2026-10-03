#!/usr/bin/env python3
"""Reopen Rust-generated JWWs in a disposable Windows/Wine Jw_cad 10.02.1 install.

Use the runtime directory and inputs created as described in docs/JWW_WRITE.md.
Only processes launched by this script are controlled. Existing outputs are never
replaced. No third-party application or drawing is distributed with this tool.
"""
import argparse
import hashlib
import json
import subprocess
import time
from pathlib import Path

import ctypes as C
from ctypes import wintypes as W

u = C.WinDLL('user32', use_last_error=True)
CALLBACK = C.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
SIGNATURES = [
    ('EnumWindows', [CALLBACK, W.LPARAM], W.BOOL),
    ('EnumChildWindows', [W.HWND, CALLBACK, W.LPARAM], W.BOOL),
    ('GetWindowTextW', [W.HWND, W.LPWSTR, C.c_int], C.c_int),
    ('GetClassNameW', [W.HWND, W.LPWSTR, C.c_int], C.c_int),
    ('GetWindowThreadProcessId', [W.HWND, C.POINTER(W.DWORD)], W.DWORD),
    ('GetDlgCtrlID', [W.HWND], C.c_int),
    ('IsWindowVisible', [W.HWND], W.BOOL),
    ('PostMessageW', [W.HWND, W.UINT, W.WPARAM, W.LPARAM], W.BOOL),
    ('SetWindowTextW', [W.HWND, W.LPCWSTR], W.BOOL),
    ('GetWindowRect', [W.HWND, C.POINTER(W.RECT)], W.BOOL),
]
for name, arguments, result in SIGNATURES:
    function = getattr(u, name)
    function.argtypes = arguments
    function.restype = result


def info(handle):
    title = C.create_unicode_buffer(2048)
    class_name = C.create_unicode_buffer(256)
    rectangle = W.RECT()
    u.GetWindowTextW(handle, title, len(title))
    u.GetClassNameW(handle, class_name, len(class_name))
    u.GetWindowRect(handle, C.byref(rectangle))
    return dict(
        h=handle, title=title.value, cls=class_name.value,
        id=u.GetDlgCtrlID(handle), visible=bool(u.IsWindowVisible(handle)),
        rect=[rectangle.left, rectangle.top, rectangle.right, rectangle.bottom],
    )


def windows(parent=None):
    result = []

    @CALLBACK
    def callback(handle, _parameter):
        if u.IsWindowVisible(handle):
            result.append(info(handle))
        return True

    if parent:
        u.EnumChildWindows(parent, callback, 0)
    else:
        u.EnumWindows(callback, 0)
    return result


EXE_SHA256 = '95e6b11c4ee014e0079f288429ae2c6e5eed41141a963e4b8770d2e8ead87acf'


class NativeSession:
    def __init__(self, runtime):
        self.runtime = runtime.resolve()
        self.process = None

    def visible(self):
        # Each operation is confined to the process started by this instance.
        import ctypes
        from ctypes import wintypes
        result = []
        for window in windows():
            pid = wintypes.DWORD()
            u.GetWindowThreadProcessId(window['h'], ctypes.byref(pid))
            if self.process is not None and pid.value == self.process.pid:
                result.append(window)
        return result

    def wait_for(self, predicate, timeout=30):
        end = time.monotonic() + timeout
        while time.monotonic() < end:
            value = predicate()
            if value:
                return value
            time.sleep(.15)
        raise RuntimeError('Native operation timed out: ' + json.dumps(self.visible()))

    def dialog(self, title):
        return next((w['h'] for w in self.visible() if w['title'] == title and w['cls'] == '#32770'), None)

    def main(self):
        return next(w['h'] for w in self.visible() if w['cls'].startswith('Afx:') and 'jw_win' in w['title'])

    @staticmethod
    def command(handle, command_id):
        if not u.PostMessageW(handle, 0x111, command_id, 0):
            raise RuntimeError('WM_COMMAND failed')

    def launch(self, path):
        self.process = subprocess.Popen([str(self.runtime / 'Jw_win.exe'), str(path)], cwd=self.runtime)

        def ready():
            for window in self.visible():
                if window['title'] == 'jw_win' and window['cls'] == '#32770':
                    children = windows(window['h'])
                    if any('関連付け' in child['title'] for child in children):
                        self.command(window['h'], 2)
                    else:
                        raise RuntimeError('Native application message: ' + json.dumps(children))
            return next((w['h'] for w in self.visible() if w['title'].startswith(path.name + ' - jw_win')), None)

        self.wait_for(ready)

    def save(self, stem, extension):
        destination = self.runtime / (stem + '.' + extension)
        if destination.exists():
            raise FileExistsError(destination)
        self.command(self.main(), {'jwc': 32810, 'jww': 57604, 'dxf': 32961}[extension])
        selector = self.wait_for(lambda: self.dialog('ファイル選択'))
        self.command(selector, 2408)
        new_file = self.wait_for(lambda: self.dialog('新規作成'))
        controls = {child['id']: child['h'] for child in windows(new_file)}
        if not u.SetWindowTextW(controls[1491], stem):
            raise RuntimeError('Could not set filename')
        self.command(new_file, 1)
        self.wait_for(lambda: not self.dialog('新規作成'))
        self.wait_for(lambda: not self.dialog('ファイル選択'))
        self.wait_for(lambda: info(self.main())['title'].startswith(stem + '.' + extension + ' - '))

        def readable():
            try:
                return destination.read_bytes()
            except (PermissionError, FileNotFoundError):
                return None

        initial = self.wait_for(readable)
        time.sleep(1)
        if initial != destination.read_bytes():
            raise RuntimeError('Output is still changing')
        if self.dialog('jw_win'):
            raise RuntimeError('Unexpected native application message')

    def close(self):
        u.PostMessageW(self.main(), 0x10, 0, 0)
        self.wait_for(lambda: not any('jw_win' in w['title'] for w in self.visible()))
        code = self.process.wait(timeout=15)
        if code:
            raise RuntimeError(f'Native process exited with {code}')
        self.process = None


def digest(path):
    data = path.read_bytes()
    return dict(size=len(data), sha256=hashlib.sha256(data).hexdigest())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', required=True, type=Path)
    parser.add_argument('--log', required=True, type=Path)
    args = parser.parse_args()
    runtime = args.runtime.resolve()
    if digest(runtime / 'Jw_win.exe')['sha256'] != EXE_SHA256:
        raise ValueError('Unverified Jw_cad executable version')
    # Check every destination before launching the first application process.
    if args.log.exists():
        raise FileExistsError(args.log)
    for name in ('empty', 'line'):
        if not (runtime / (name + '.jww')).is_file():
            raise FileNotFoundError(name + '.jww')
        for suffix in ('_saved.jww', '_reopened.jww', '_reopened.dxf'):
            if (runtime / (name + suffix)).exists():
                raise FileExistsError(name + suffix)
    session = NativeSession(runtime)
    events = []
    try:
        for name in ('empty', 'line'):
            source = runtime / (name + '.jww')
            before = digest(source)
            session.launch(source)
            session.save(name + '_saved', 'jww')
            session.close()
            session.launch(runtime / (name + '_saved.jww'))
            session.save(name + '_reopened', 'jww')
            session.save(name + '_reopened', 'dxf')
            session.close()
            if digest(source) != before:
                raise RuntimeError('Input changed: ' + name)
            paths = [source] + [runtime / (name + suffix) for suffix in
                               ('_saved.jww', '_reopened.jww', '_reopened.dxf')]
            events.append(dict(id=name, status='save_reopen_save_export',
                               exit_codes=[0, 0], files={p.name: digest(p) for p in paths}))
            print('verified native reopen: ' + name, flush=True)
    finally:
        # A failure must not leave our own CAD process running.
        if session.process is not None and session.process.poll() is None:
            session.process.terminate()
            session.process.wait(timeout=15)
    with args.log.open('x', encoding='utf-8') as output:
        json.dump(dict(executable_sha256=EXE_SHA256, cases=events), output, indent=2)
        output.write('\n')


if __name__ == '__main__':
    main()
