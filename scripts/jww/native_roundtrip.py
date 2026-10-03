#!/usr/bin/env python3
"""Reopen Rust-generated JWWs in a disposable Windows/Wine Jw_cad 10.02.1 install.

Use the runtime directory and inputs created as described in docs/JWW_WRITE.md.
Only processes launched by this script are controlled. Existing outputs are never
replaced. No third-party application or drawing is distributed with this tool.
"""
import argparse
import hashlib
import json
import platform
import struct
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

from contextlib import contextmanager
import winreg

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
        main = next((w['h'] for w in self.visible() if w['cls'].startswith('Afx:') and 'jw_win' in w['title']), None)
        if main is None:
            raise RuntimeError(f'Native main window disappeared; process exit: {self.process.poll()}')
        return main

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
        return code

    def screenshot(self, path):
        """Capture this process's visible client area as an uncompressed BMP."""
        if path.exists():
            raise FileExistsError(path)
        handle = self.main()
        gdi = C.WinDLL('gdi32', use_last_error=True)
        signatures = [
            (u, 'GetClientRect', [W.HWND, C.POINTER(W.RECT)], W.BOOL),
            (u, 'GetDC', [W.HWND], W.HDC),
            (u, 'ReleaseDC', [W.HWND, W.HDC], C.c_int),
            (u, 'MoveWindow', [W.HWND, C.c_int, C.c_int, C.c_int, C.c_int, W.BOOL], W.BOOL),
            (u, 'RedrawWindow', [W.HWND, C.c_void_p, W.HANDLE, W.UINT], W.BOOL),
            (gdi, 'CreateCompatibleDC', [W.HDC], W.HDC),
            (gdi, 'DeleteDC', [W.HDC], W.BOOL),
            (gdi, 'CreateCompatibleBitmap', [W.HDC, C.c_int, C.c_int], W.HBITMAP),
            (gdi, 'SelectObject', [W.HDC, W.HANDLE], W.HANDLE),
            (gdi, 'DeleteObject', [W.HANDLE], W.BOOL),
            (gdi, 'BitBlt', [W.HDC, C.c_int, C.c_int, C.c_int, C.c_int,
                             W.HDC, C.c_int, C.c_int, W.DWORD], W.BOOL),
            (gdi, 'GetDIBits', [W.HDC, W.HBITMAP, W.UINT, W.UINT,
                               C.c_void_p, C.c_void_p, W.UINT], C.c_int),
        ]
        for library, name, arguments, result in signatures:
            function = getattr(library, name)
            function.argtypes, function.restype = arguments, result
        # Fix the viewport and force a repaint before measuring the client.
        # Without a window manager Wine can report a resized client while the
        # application's old-size drawing surface is still visible.
        if not u.MoveWindow(handle, 0, 0, 1260, 860, True):
            raise C.WinError(C.get_last_error())
        u.RedrawWindow(handle, None, None, 0x0001 | 0x0004 | 0x0080 | 0x0100)
        time.sleep(.5)
        rect = W.RECT()
        if not u.GetClientRect(handle, C.byref(rect)):
            raise C.WinError(C.get_last_error())
        width, height = rect.right, rect.bottom
        dc = u.GetDC(handle)
        memory = gdi.CreateCompatibleDC(dc)
        bitmap = gdi.CreateCompatibleBitmap(dc, width, height)
        if not dc or not memory or not bitmap:
            raise RuntimeError('Could not allocate screenshot bitmap')
        old = gdi.SelectObject(memory, bitmap)
        try:
            time.sleep(.5)  # Let Jw_cad finish repainting after load/save.
            if not gdi.BitBlt(memory, 0, 0, width, height, dc, 0, 0, 0x00CC0020):
                raise C.WinError(C.get_last_error())
            gdi.SelectObject(memory, old)
            pixels = C.create_string_buffer(width * height * 4)
            header = struct.pack('<IiiHHIIiiII', 40, width, height, 1, 32, 0, len(pixels), 0, 0, 0, 0)
            bmi = C.create_string_buffer(header)
            if gdi.GetDIBits(dc, bitmap, 0, height, pixels, bmi, 0) != height:
                raise RuntimeError('Incomplete screenshot capture')
            with path.open('xb') as output:
                output.write(struct.pack('<2sIHHI', b'BM', 54 + len(pixels), 0, 0, 54))
                output.write(header)
                output.write(pixels.raw)
        finally:
            gdi.SelectObject(memory, old)
            gdi.DeleteObject(bitmap)
            gdi.DeleteDC(memory)
            u.ReleaseDC(handle, dc)


def digest(path):
    data = path.read_bytes()
    return dict(size=len(data), sha256=hashlib.sha256(data).hexdigest())


def environment():
    runtime = dict(kind='windows', windows_version=platform.platform(),
                   python_version=sys.version, ansi_code_page=C.windll.kernel32.GetACP())
    try:
        version = C.CDLL('ntdll').wine_get_version
    except AttributeError:
        pass
    else:
        version.restype = C.c_char_p
        runtime.update(kind='wine', wine_version=version().decode('ascii'))
    return runtime


@contextmanager
def output_folders(runtime):
    # JWW and DXF remember separate folders, even after opening an absolute path.
    # Use only a disposable desktop/prefix and restore these preferences on exit.
    if any(w['cls'].startswith('Afx:') and 'jw_win' in w['title'] for w in windows()):
        raise RuntimeError('Close other Jw_cad processes before native validation')
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, r'Software\Jw_cad\jw_win\Folder') as key:
        previous = {}
        try:
            for name in ('File', 'FileC'):
                try:
                    previous[name] = winreg.QueryValueEx(key, name)
                except FileNotFoundError:
                    previous[name] = None
                winreg.SetValueEx(key, name, 0, winreg.REG_SZ, str(runtime))
            yield
        finally:
            for name, old in previous.items():
                if old is None:
                    try:
                        winreg.DeleteValue(key, name)
                    except FileNotFoundError:
                        pass
                else:
                    value, kind = old
                    winreg.SetValueEx(key, name, 0, kind, value)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--runtime', required=True, type=Path)
    parser.add_argument('--log', required=True, type=Path)
    parser.add_argument('--cases', nargs='+', default=['empty', 'line'])
    parser.add_argument('--jww-only', nargs='*', default=[], help='Cases to reopen/save without native DXF export')
    parser.add_argument('--screenshots', nargs='*', default=[], help='Capture client BMP before and after native save')
    args = parser.parse_args()
    if len(set(args.cases)) != len(args.cases) or any(
        not name or any(c not in 'abcdefghijklmnopqrstuvwxyz0123456789_' for c in name)
        for name in args.cases
    ):
        raise ValueError('Case IDs must be unique lowercase ASCII basenames')
    if not set(args.jww_only).issubset(args.cases):
        raise ValueError('--jww-only must name selected cases')
    if not set(args.screenshots).issubset(args.cases):
        raise ValueError('--screenshots must name selected cases')
    runtime = args.runtime.resolve()
    if digest(runtime / 'Jw_win.exe')['sha256'] != EXE_SHA256:
        raise ValueError('Unverified Jw_cad executable version')
    # Check every destination before launching the first application process.
    if args.log.exists():
        raise FileExistsError(args.log)
    for name in args.cases:
        if not (runtime / (name + '.jww')).is_file():
            raise FileNotFoundError(name + '.jww')
        for suffix in ('_saved.jww', '_reopened.jww', '_reopened.dxf', '_opened.bmp', '_reopened.bmp'):
            if (runtime / (name + suffix)).exists():
                raise FileExistsError(name + suffix)
    session = NativeSession(runtime)
    events = []
    report = dict(executable_sha256=EXE_SHA256, environment=environment(),
                  script_sha256=digest(Path(__file__))["sha256"],
                  started_at_utc=datetime.now(timezone.utc).isoformat(), cases=events)
    try:
        with output_folders(runtime):
            for name in args.cases:
                source = runtime / (name + '.jww')
                before = digest(source)
                event = dict(id=name, status='in_progress', stages=[], exit_codes=[], files={})
                events.append(event)
                try:
                    session.launch(source)
                    event['stages'].append('opened')
                    if name in args.screenshots:
                        session.screenshot(runtime / (name + '_opened.bmp'))
                    session.save(name + '_saved', 'jww')
                    event['stages'].append('saved')
                    event['exit_codes'].append(session.close())
                    session.launch(runtime / (name + '_saved.jww'))
                    event['stages'].append('reopened')
                    if name in args.screenshots:
                        session.screenshot(runtime / (name + '_reopened.bmp'))
                    session.save(name + '_reopened', 'jww')
                    event['stages'].append('resaved')
                    if name not in args.jww_only:
                        session.save(name + '_reopened', 'dxf')
                        event['stages'].append('dxf_exported')
                    event['exit_codes'].append(session.close())
                    if digest(source) != before:
                        raise RuntimeError('Input changed: ' + name)
                    event['status'] = 'save_reopen_save' if name in args.jww_only else 'save_reopen_save_export'
                except Exception as error:
                    event.update(status='failed', error=str(error))
                    if session.process is not None:
                        event['process_exit_code'] = session.process.poll()
                    raise
                finally:
                    event['input_unchanged'] = digest(source) == before
                    paths = [source] + [runtime / (name + suffix) for suffix in
                                       ('_saved.jww', '_reopened.jww', '_reopened.dxf',
                                        '_opened.bmp', '_reopened.bmp')]
                    event['files'] = {p.name: digest(p) for p in paths if p.is_file()}
                print('verified native reopen: ' + name, flush=True)
    finally:
        # Retain partial evidence even when a later native operation fails.
        try:
            if session.process is not None and session.process.poll() is None:
                session.process.terminate()
                session.process.wait(timeout=15)
        finally:
            report['finished_at_utc'] = datetime.now(timezone.utc).isoformat()
            with args.log.open('x', encoding='utf-8') as output:
                json.dump(report, output, indent=2)
                output.write('\n')


if __name__ == '__main__':
    main()
