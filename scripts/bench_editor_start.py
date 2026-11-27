#!/usr/bin/env python3
"""Compare normal Editor launches through the first composed Win32 toolbar."""

import argparse
import ctypes as c
from ctypes import wintypes as w
import os
from pathlib import Path
import platform
import re
import struct
import subprocess
import sys
import time

from bench import COMPARE_ROUNDS, command_output, judge_metric, write_json
from editor_diagnostics import DiagnosticError, png_evidence
from review_snapshot import entry


TITLE = "ModuleEngine / Engine Constructor"
CLIENT = (1580, 940)
WIDTH, HEIGHT = 170, 26
MENUS = ((8, 34), (37, 64), (67, 103), (105, 166))


def digest(path):
    return entry(path.parent, path.name)["sha256"]


def fixture_inventory(project):
    return {path.relative_to(project.parent).as_posix(): digest(path)
            for path in sorted(project.parent.rglob("*")) if path.is_file()}


def require_diagnostic_editor(editor, root):
    help_result = subprocess.run([str(editor), "--help"], cwd=root, capture_output=True, timeout=15)
    # R0.6 includes diagnostic personal settings in its regular build. The
    # current Editor exposes this same option only in its verification build.
    if help_result.returncode != 0 or b"module-engine-editor --editor-benchmark " not in help_result.stdout:
        raise ValueError(f"{editor} does not expose the diagnostic Editor routes. "
                         "Use a Release build with diagnostic personal settings (verification on current sources).")


def toolbar_visible(pixels):
    # The default 100% DPI menu bar always draws these four labels in this band.
    # The window background and a cleared or blank swapchain contain none.
    for left, right in MENUS:
        bright = dark = 0
        for y in range(6, 21):
            for x in range(left, right):
                i = (y * WIDTH + x) * 4
                blue, green, red = pixels[i:i + 3]
                bright += min(blue, green, red) >= 150
                dark += max(blue, green, red) < 80
        if bright < 12 or dark < (right - left) * 8:
            return False
    return True


def bitmap_header():
    return struct.pack("<IiiHHIIiiII", 40, WIDTH, -HEIGHT, 1, 32, 0, 0, 0, 0, 0, 0)


def save_bitmap(path, pixels):
    header = bitmap_header()
    path.write_bytes(struct.pack("<2sIHHI", b"BM", 14 + len(header) + len(pixels), 0, 0, 14 + len(header))
                     + header + pixels)


class Desktop:
    def __init__(self):
        user, gdi = c.WinDLL("user32", use_last_error=True), c.WinDLL("gdi32", use_last_error=True)
        callback = c.WINFUNCTYPE(w.BOOL, w.HWND, w.LPARAM)
        self.callback = callback
        for dll, name, returns, args in (
            (user, "SetProcessDpiAwarenessContext", w.BOOL, (w.HANDLE,)),
            (user, "EnumWindows", w.BOOL, (callback, w.LPARAM)),
            (user, "GetWindowThreadProcessId", w.DWORD, (w.HWND, c.POINTER(w.DWORD))),
            (user, "IsWindowVisible", w.BOOL, (w.HWND,)),
            (user, "GetWindowTextW", c.c_int, (w.HWND, w.LPWSTR, c.c_int)),
            (user, "GetClientRect", w.BOOL, (w.HWND, c.POINTER(w.RECT))),
            (user, "ClientToScreen", w.BOOL, (w.HWND, c.POINTER(w.POINT))),
            (user, "GetForegroundWindow", w.HWND, ()),
            (user, "GetDpiForWindow", w.UINT, (w.HWND,)),
            (user, "MonitorFromWindow", w.HANDLE, (w.HWND, w.DWORD)),
            (user, "WindowFromPoint", w.HWND, (w.POINT,)),
            (user, "PostMessageW", w.BOOL, (w.HWND, w.UINT, w.WPARAM, w.LPARAM)),
            (user, "SetWindowPos", w.BOOL, (w.HWND, w.HWND, c.c_int, c.c_int, c.c_int, c.c_int, w.UINT)),
            (user, "GetDC", w.HDC, (w.HWND,)),
            (user, "ReleaseDC", c.c_int, (w.HWND, w.HDC)),
            (gdi, "CreateCompatibleDC", w.HDC, (w.HDC,)),
            (gdi, "DeleteDC", w.BOOL, (w.HDC,)),
            (gdi, "CreateDIBSection", w.HBITMAP,
             (w.HDC, c.c_void_p, w.UINT, c.POINTER(c.c_void_p), w.HANDLE, w.DWORD)),
            (gdi, "SelectObject", w.HANDLE, (w.HDC, w.HANDLE)),
            (gdi, "DeleteObject", w.BOOL, (w.HANDLE,)),
            (gdi, "BitBlt", w.BOOL,
             (w.HDC, c.c_int, c.c_int, c.c_int, c.c_int, w.HDC, c.c_int, c.c_int, w.DWORD)),
            (gdi, "GdiFlush", w.BOOL, ()),
        ):
            function = getattr(dll, name)
            function.restype, function.argtypes = returns, args
        if not user.SetProcessDpiAwarenessContext(w.HANDLE(-4)):
            raise c.WinError(c.get_last_error())
        self.user, self.gdi = user, gdi
        self.screen = self.buffer = self.bitmap = self.previous = None
        self.address = c.c_void_p()
        self.last_window = None

    def __enter__(self):
        try:
            self.screen = self.user.GetDC(None)
            if not self.screen:
                raise c.WinError(c.get_last_error())
            self.buffer = self.gdi.CreateCompatibleDC(self.screen)
            info = c.create_string_buffer(bitmap_header())
            self.bitmap = self.gdi.CreateDIBSection(self.screen, info, 0, c.byref(self.address), None, 0)
            if not self.buffer or not self.bitmap or not self.address:
                raise c.WinError(c.get_last_error())
            self.previous = self.gdi.SelectObject(self.buffer, self.bitmap)
            if not self.previous or self.previous == w.HANDLE(-1).value:
                self.previous = None
                raise c.WinError(c.get_last_error())
            return self
        except OSError:
            self.__exit__(None, None, None)
            raise

    def __exit__(self, *_):
        if self.previous:
            self.gdi.SelectObject(self.buffer, self.previous)
        if self.bitmap:
            self.gdi.DeleteObject(self.bitmap)
        if self.buffer:
            self.gdi.DeleteDC(self.buffer)
        if self.screen:
            self.user.ReleaseDC(None, self.screen)

    def window(self, pid):
        matches = []

        @self.callback
        def visit(hwnd, _):
            owner = w.DWORD()
            self.user.GetWindowThreadProcessId(hwnd, c.byref(owner))
            if owner.value == pid and self.user.IsWindowVisible(hwnd):
                title = c.create_unicode_buffer(len(TITLE) + 2)
                self.user.GetWindowTextW(hwnd, title, len(title))
                if title.value == TITLE:
                    matches.append(hwnd)
            return True

        if not self.user.EnumWindows(visit, 0):
            raise c.WinError(c.get_last_error())
        if len(matches) > 1:
            raise ValueError("More than one visible Editor root belongs to the measured process")
        return matches[0] if matches else None

    def pixels(self, hwnd):
        client, origin = w.RECT(), w.POINT()
        if not self.user.GetClientRect(hwnd, c.byref(client)) or not self.user.ClientToScreen(hwnd, c.byref(origin)):
            raise c.WinError(c.get_last_error())
        dpi = self.user.GetDpiForWindow(hwnd)
        foreground = self.user.GetForegroundWindow() == hwnd
        self.last_window = {"client": [client.right, client.bottom], "dpi": dpi,
                            "origin": [origin.x, origin.y], "foreground": foreground,
                            "monitor": self.user.MonitorFromWindow(hwnd, 0)}
        if (client.right, client.bottom) != CLIENT or dpi != 96:
            return None
        # Check the corners of the sampled label band, inside the native border.
        points = [w.POINT(origin.x + x, origin.y + y) for x in (MENUS[0][0], MENUS[-1][1] - 1)
                  for y in (6, 20)]

        def unobstructed():
            visible = all(self.user.WindowFromPoint(point) == hwnd for point in points)
            self.last_window["label_band_unobstructed"] = visible
            return visible

        if not unobstructed():
            return None
        if not self.gdi.BitBlt(self.buffer, 0, 0, WIDTH, HEIGHT, self.screen, origin.x, origin.y, 0x00CC0020):
            raise c.WinError(c.get_last_error())
        if not self.gdi.GdiFlush():
            raise c.WinError(c.get_last_error())
        return c.string_at(self.address, WIDTH * HEIGHT * 4) if unobstructed() else None


def measure(desktop, editor, root, fixture, folder):
    folder.mkdir()
    personal = folder / "personal"
    personal.mkdir()
    env = {key: value for key, value in os.environ.items() if not key.startswith((
        "MODULE_ENGINE_CAPTURE_", "MODULE_ENGINE_GUI_", "MODULE_ENGINE_UI_"))
        and key not in ("EFRAME_SCREENSHOT_TO", "MODULE_ENGINE_SHOW_CUSTOM", "MODULE_ENGINE_WINDOW_SYSTEM")}
    env["MODULE_ENGINE_DIAGNOSTIC_USER_ROOT"] = str(personal)
    # The regular window opens normally. The existing UI capture route closes
    # it after several frames and a settling delay, after the measured event.
    capture = folder / "shutdown-frame.png"
    env["MODULE_ENGINE_UI_SCREENSHOT_TO"] = str(capture)
    with (folder / "stdout.txt").open("wb") as stdout, (folder / "stderr.txt").open("wb") as stderr:
        start = time.perf_counter_ns()
        process = subprocess.Popen([str(editor), str(fixture)], cwd=root, env=env, stdout=stdout, stderr=stderr)
        hwnd = None
        raised = None
        raise_count = 0
        lower = start
        observed_pixels = None
        first_frame = False
        forced_close = False
        desktop.last_window = None
        try:
            while time.perf_counter_ns() - start < 30_000_000_000 and process.poll() is None:
                before = time.perf_counter_ns()
                hwnd = desktop.window(process.pid)
                covered = desktop.last_window and desktop.last_window.get("label_band_unobstructed") is False
                if hwnd and (hwnd != raised or covered):
                    # Put the measured window above other applications without
                    # stealing keyboard focus or changing its size/position.
                    if not desktop.user.SetWindowPos(hwnd, w.HWND(-1), 0, 0, 0, 0, 0x13):
                        raise c.WinError(c.get_last_error())
                    raised = hwnd
                    raise_count += 1
                pixels = desktop.pixels(hwnd) if hwnd else None
                after = time.perf_counter_ns()
                if pixels:
                    observed_pixels = pixels
                if pixels and toolbar_visible(pixels):
                    first_frame = True
                    break
                lower = before
                time.sleep(0.002)
            else:
                if observed_pixels:
                    save_bitmap(folder / "last-sampled-toolbar.bmp", observed_pixels)
                raise ValueError(f"First Editor toolbar not observed for pid {process.pid}: "
                                 f"{desktop.last_window}, {folder}")
        finally:
            if process.poll() is None:
                if not first_frame and hwnd:
                    desktop.user.PostMessageW(hwnd, 0x0010, 0, 0)  # WM_CLOSE
                try:
                    process.wait(timeout=5)
                except subprocess.TimeoutExpired:
                    forced_close = True
                    taskkill = Path(os.environ.get("SystemRoot", "C:/Windows")) / "System32/taskkill.exe"
                    cleanup = subprocess.run([str(taskkill), "/PID", str(process.pid), "/T", "/F"],
                                             capture_output=True, timeout=15)
                    (folder / "cleanup.log").write_bytes(cleanup.stdout + cleanup.stderr)
                    if cleanup.returncode and process.poll() is None:
                        process.terminate()
                    process.wait(timeout=5)
    save_bitmap(folder / "first-toolbar.bmp", pixels)
    log = (folder / "stderr.txt").read_text(encoding="utf-8", errors="replace")
    graphics = re.findall(r'ModuleEngine GUI initialized: (GuiGraphicsInfo \{.*\})', log)
    if len(graphics) != 1 or 'window_system: Some("Windows")' not in graphics[0]:
        raise ValueError(f"Native window graphics identity missing: {folder}")
    adapter = re.search(r'adapter_name: Some\("([^"]+)"\)', graphics[0])
    if not adapter:
        raise ValueError(f"Native window adapter missing: {folder}")
    record = {"lower_ms": (lower - start) / 1e6, "upper_ms": (after - start) / 1e6,
              "exit_code": process.returncode, "forced_close": forced_close,
              "graphics": graphics[0], "adapter": adapter[1], "window": desktop.last_window,
              "toolbar_sha256": digest(folder / "first-toolbar.bmp"), "raise_count": raise_count}
    write_json(folder / "launch.json", record)
    if forced_close or process.returncode != 0:
        raise ValueError(f"Editor did not shut down cleanly: {folder}")
    evidence = png_evidence(capture, folder)
    if [evidence["width"], evidence["height"]] != list(CLIENT):
        raise ValueError(f"Unexpected native screenshot dimensions: {folder}")
    write_json(folder / "shutdown-frame.json", evidence)
    return record


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for side in ("baseline", "candidate"):
        parser.add_argument(f"--{side}-editor", type=Path, required=True)
        parser.add_argument(f"--{side}-root", type=Path, required=True)
    parser.add_argument("--project", type=Path, required=True,
                        help="The same project in a disposable fixture folder for both executables")
    parser.add_argument("--out", type=Path, required=True, help="A new output directory")
    args = parser.parse_args()
    if os.name != "nt":
        parser.error("Requires an unlocked native Windows desktop at 100% DPI")
    sides = {
        name: (getattr(args, f"{name}_editor").resolve(strict=True),
               getattr(args, f"{name}_root").resolve(strict=True))
        for name in ("baseline", "candidate")
    }
    fixture, output = args.project.resolve(strict=True), args.out.resolve()
    if output.is_relative_to(fixture.parent):
        parser.error("The output directory must be outside the input fixture directory")
    for editor, root in sides.values():
        require_diagnostic_editor(editor, root)
    fixture_before = fixture_inventory(fixture)
    output.mkdir(parents=True)
    executables = {name: digest(editor) for name, (editor, _) in sides.items()}
    write_json(output / "inputs.json", {
        "project": str(fixture), "fixture_sha256": fixture_before,
        "sides": {name: {"editor": str(editor), "sha256": executables[name], "root": str(root)}
                  for name, (editor, root) in sides.items()},
        "system": platform.platform(),
        "nvidia_smi": command_output("nvidia-smi", "--query-gpu=name,driver_version", "--format=csv,noheader"),
    })
    samples = {name: [] for name in sides}
    with Desktop() as desktop:
        def run(name, label):
            case = output / f"{label}-{name}"
            result = measure(desktop, *sides[name], fixture, case)
            print(case.name, f'{result["lower_ms"]:.2f} to {result["upper_ms"]:.2f} ms', flush=True)
            return result

        prewarm = {name: run(name, "prewarm") for name in sides}
        for session, order in ((1, tuple(sides)), (2, tuple(reversed(sides)))):
            for index in range(1, COMPARE_ROUNDS + 1):
                for name in order:
                    samples[name].append(run(name, f"{session}-{index}"))
            baseline = [row["upper_ms"] for row in samples["baseline"]]
            candidate = [row["upper_ms"] for row in samples["candidate"]]
            verdict = judge_metric("timing", baseline, candidate, True)
            if not verdict["borderline"]:
                break
    fixtures_match = fixture_before == fixture_inventory(fixture)
    binaries_match = executables == {name: digest(editor) for name, (editor, _) in sides.items()}
    records = [*prewarm.values(), *(row for case in samples.values() for row in case)]
    same_adapter = len({row["adapter"] for row in records}) == 1
    monitors = {row["window"]["monitor"] for row in records}
    same_monitor = None not in monitors and len(monitors) == 1
    stable_backends = all(len({prewarm[name]["graphics"], *(row["graphics"] for row in samples[name])}) == 1
                          for name in sides)
    accepted = all((fixtures_match, binaries_match, same_adapter, same_monitor, stable_backends,
                    verdict["verdict"] in ("better", "same")))
    write_json(output / "compare.json", {
        "accepted": accepted, "fixture_unchanged": fixtures_match, "binaries_unchanged": binaries_match,
        "same_adapter": same_adapter, "same_monitor": same_monitor, "stable_backends": stable_backends,
        "client_pixels": CLIENT, "dpi": 96,
        "start": "immediately before CreateProcess for the ordinary Editor command",
        "end": "first sampled desktop pixels containing all four root toolbar labels",
        "prewarm": prewarm, "samples": samples, "metric": verdict,
    })
    return 0 if accepted else 1


if __name__ == "__main__":
    try:
        sys.exit(main())
    except (OSError, ValueError, DiagnosticError, subprocess.SubprocessError) as error:
        sys.exit(str(error))
