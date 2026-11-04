#!/usr/bin/env python3
"""Opt-in real Chess3D pixel/input smoke on an isolated Xvfb display (never desktop).

Requires Pillow, xdotool, libX11 and a chess3d binary built with --features viewer.
The caller starts Xvfb, sets DISPLAY and chooses Vulkan ICD/library paths. Each
window is selected by child PID. Capture references exercise the same real game scene.
"""

import argparse
from collections import deque
import ctypes as c
import json
import os
from pathlib import Path
import subprocess
import tempfile
import time

from PIL import Image, ImageChops, ImageGrab, ImageStat

DISPLAY = os.environ["DISPLAY"]
STALL_TIMEOUT_SECONDS = 180
CAPTURE_TIMEOUT_SECONDS = 1200
CASES = {
    "hero": {"view": "hero", "interaction": "camera"},
    "overhead": {"view": "overhead"},
    "title-to-friend": {"view": "overhead", "interaction": "title"},
    "computer-level-1": {"view": "overhead", "interaction": "computer"},
    "study": {"view": "hero", "load": Path(__file__).resolve().parents[1] / "assets/positions/study.json"},
    "lineup": {"view": "lineup"},
}


class CaseProgress:
    def __init__(self, name, output):
        self.name = name
        self.output = output
        self.started = time.monotonic()

    def elapsed(self):
        return time.monotonic() - self.started

    def __call__(self, label):
        line = f"[window case={self.name} elapsed={self.elapsed():.2f}s] {label}"
        print(line, flush=True)
        with (self.output / "progress.log").open("a") as stream:
            stream.write(line + "\n")


def wait_for(predicate, label, *, process, progress):
    def check_running():
        status = process.poll()
        if status is not None:
            log_path = progress.output / f"{progress.name}.log"
            with log_path.open(errors="replace") as stream:
                tail = "".join(deque(stream, maxlen=20))
            raise AssertionError(
                f"Chess process exited with code {status} waiting for {label} "
                f"(case {progress.name})\n{log_path} (last 20 lines):\n{tail}")

    deadline = time.monotonic() + STALL_TIMEOUT_SECONDS
    progress(f"wait: {label}")
    while time.monotonic() < deadline:
        check_running()
        try:
            result = predicate()
        finally:
            check_running()
        if result:
            progress(f"ready: {label}")
            return result
        time.sleep(0.05)
    check_running()
    raise AssertionError(
        f"Timed out waiting for {label} (case {progress.name}) after {STALL_TIMEOUT_SECONDS}s without progress")


def difference(left, right):
    if left.size != right.size:
        return float("inf")
    return sum(ImageStat.Stat(ImageChops.difference(left, right)).mean) / 3


def close_request(window):
    # Send the same WM_PROTOCOLS message as a window-manager close button.
    class Data(c.Union):
        _fields_ = [("longs", c.c_long * 5)]

    class ClientMessage(c.Structure):
        _fields_ = [("type", c.c_int), ("serial", c.c_ulong),
                    ("send_event", c.c_int), ("display", c.c_void_p),
                    ("window", c.c_ulong), ("message_type", c.c_ulong),
                    ("format", c.c_int), ("data", Data)]

    class Event(c.Union):
        _fields_ = [("client", ClientMessage), ("padding", c.c_long * 24)]

    x11 = c.CDLL("libX11.so.6")
    x11.XOpenDisplay.argtypes = [c.c_char_p]
    x11.XOpenDisplay.restype = c.c_void_p
    x11.XInternAtom.argtypes = [c.c_void_p, c.c_char_p, c.c_int]
    x11.XInternAtom.restype = c.c_ulong
    x11.XSendEvent.argtypes = [c.c_void_p, c.c_ulong, c.c_int, c.c_long, c.POINTER(Event)]
    x11.XFlush.argtypes = [c.c_void_p]
    x11.XCloseDisplay.argtypes = [c.c_void_p]
    display = x11.XOpenDisplay(DISPLAY.encode())
    assert display, "isolated Xvfb unavailable"
    try:
        event = Event()
        event.client.type = 33
        event.client.display = display
        event.client.window = int(window)
        event.client.message_type = x11.XInternAtom(display, b"WM_PROTOCOLS", 0)
        event.client.format = 32
        event.client.data.longs[0] = x11.XInternAtom(display, b"WM_DELETE_WINDOW", 0)
        assert x11.XSendEvent(display, int(window), 0, 0, c.byref(event)) != 0
        x11.XFlush(display)
    finally:
        x11.XCloseDisplay(display)


def run_case(args, environment, name, view, load=None, interaction=None):
    progress = CaseProgress(name, args.output)
    progress("start")
    outcome = "failed"
    try:
        with tempfile.TemporaryDirectory(prefix="chess3d-window-data-") as data_dir:
            settings = {"view": view, "load": load, "interaction": interaction}
            result = run_isolated_case(args, environment, settings, data_dir, progress)
        result["elapsed_seconds"] = round(progress.elapsed(), 3)
        outcome = "passed"
        return result
    finally:
        progress(f"end {outcome}")


def run_isolated_case(args, environment, settings, data_dir, progress):
    return SmokeCase(args, environment, settings, data_dir, progress).run()


class SmokeCase:
    def __init__(self, args, environment, settings, data_dir, progress):
        self.args = args
        self.environment = environment
        self.progress = progress
        self.name = progress.name
        self.view = settings["view"]
        self.load = settings["load"]
        self.interaction = settings["interaction"]
        self.data_dir = data_dir
        self.computer = self.interaction == "computer"
        self.output = args.output
        self.saved = self.output / f"{self.name}-position.json"
        self.width, self.height = (1200, 600) if self.view == "lineup" else (960, 720)
        self.common = ["--view", self.view, "--width", str(self.width), "--height", str(self.height),
                       "--data-dir", data_dir]
        if self.computer:
            self.common += ["--seed", "42"]
        if self.load:
            self.common += ["--load", str(self.load)]
        self.headless = dict(environment)
        for key in ["DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"]:
            self.headless.pop(key, None)

    def capture(self, path, *arguments):
        subprocess.run([str(self.args.binary), *self.common, *arguments, "--capture", str(path)],
                       env=self.headless, check=True, capture_output=True,
                       timeout=CAPTURE_TIMEOUT_SECONDS)
        return Image.open(path).convert("RGB")

    def xdo(self, *arguments, check=True):
        return subprocess.run([str(self.args.xdotool), *map(str, arguments)],
                              env=self.environment, capture_output=True, text=True,
                              check=check, timeout=10).stdout.strip()

    def wait(self, predicate, label):
        return wait_for(predicate, label, process=self.process, progress=self.progress)

    def wait_exit(self, timeout, label):
        self.progress(f"wait: {label}")
        return self.process.wait(timeout=timeout)

    def find_window(self, label):
        self.window = self.wait(lambda: self.xdo("search", "--pid", self.process.pid, "--name",
                                                "^Chess3D", check=False), label)
        self.xdo("windowmove", self.window, 0, 0)
        self.xdo("windowfocus", self.window)

    def screenshot(self):
        fields = dict(line.split("=", 1) for line in
                      self.xdo("getwindowgeometry", "--shell", self.window).splitlines())
        x, y, width, height = [int(fields[key]) for key in ["X", "Y", "WIDTH", "HEIGHT"]]
        return ImageGrab.grab(bbox=(x, y, x + width, y + height), xdisplay=DISPLAY)

    def matching(self, reference_image, tolerance=0.1):
        image = self.screenshot()
        return image if difference(reference_image, image) <= tolerance else None

    def title_ready(self):
        image = self.screenshot()
        outside = (0, 0, 80, self.height)
        return image if (difference(self.expected.crop(outside), image.crop(outside)) <= 0.1
                         and difference(self.expected, image) > 2) else None

    def initial_state(self):
        self.records = []
        if self.interaction == "title":
            title = self.wait(self.title_ready, "title menu over retained board")
            title.save(self.output / f"{self.name}-title.png")
            self.xdo("key", "--window", self.window, "Return")
            self.records.append("title Enter starts Play a friend")
        self.initial = self.wait(lambda: self.matching(self.expected),
                                 "window matching headless chess capture")
        self.initial.save(self.output / f"{self.name}-initial.png")
        self.colors = self.initial.getcolors(2_000_000)
        assert self.colors and len(self.colors) > 1000, "missing textured chess content"
        # The wide lineup has large empty studio margins; check its subject band.
        subject = (self.initial.crop((0, self.height // 3, self.width, 3 * self.height // 4))
                   if self.view == "lineup" else self.initial)
        assert min(ImageStat.Stat(subject).stddev) > 25, "blank/flat chess subject"
        self.position = json.loads(self.saved.read_text())
        if self.load:
            assert self.position == json.loads(self.load.read_text()), "loaded position changed"
        else:
            assert len(self.position["pieces"]) == 32
            assert self.position["seed"] == (42 if self.computer else 342)
        self.saved_bytes = self.saved.read_bytes()

    def play_and_undo(self, log):
        sequence = "e2e4 b8c6" if self.computer else "e2e4"
        self.progress(f"launch: headless {sequence} capture")
        reference = self.capture(self.output / f"{self.name}-e2e4-capture.png", "--moves", sequence)
        # Engine-projected board-plane centers at this fixed 960x720 preset.
        self.xdo("mousemove", "--window", self.window, 516, 538)
        self.xdo("click", 1)
        self.wait(lambda: difference(self.initial, self.screenshot()) > 0.01,
                  "e2 legal-target highlights")
        self.xdo("mousemove", "--window", self.window, 515, 395)
        self.xdo("click", 1)
        moved = self.wait(lambda: self.matching(reference),
                          f"click and reply {sequence} matching rules-driven capture")
        moved.save(self.output / f"{self.name}-clicked-e2e4.png")
        if self.interaction == "title":
            self.relaunch(log, moved)
        self.xdo("key", "--window", self.window, "u")
        self.wait(lambda: self.matching(self.initial, 0), "U exact board undo")
        self.records += [f"click and reply {sequence} matches capture", "U exact board undo"]

    def relaunch(self, log, moved):
        self.progress("close: first chess window")
        close_request(self.window)
        assert self.wait_exit(15, "first chess window exit") == 0
        self.progress("launch: relaunch chess window with saved data")
        self.process = subprocess.Popen([str(self.args.binary), *self.common, "--window"],
                                        env=self.environment, stdout=log, stderr=log)
        self.find_window("relaunch window")
        self.wait(self.title_ready, "relaunch title menu")
        self.progress("input: click title Continue")
        self.xdo("mousemove", "--window", self.window, self.width // 2 - 196, self.height // 2 + 210)
        self.xdo("click", 1)
        self.wait(lambda: self.matching(moved, 0), "Continue exact pre-relaunch pixels")
        self.records.append("relaunch Continue restores exact board pixels")

    def puzzle_click(self):
        self.progress("inspect: puzzle 00008 projected solver coordinates")
        inspect = subprocess.run([str(self.args.binary), "--puzzle", "00008"],
                                 env=self.headless, check=True, capture_output=True,
                                 text=True, timeout=CAPTURE_TIMEOUT_SECONDS).stdout
        line = next(line for line in inspect.splitlines()
                    if line.startswith("Window clicks (960x720):"))
        fx, fy, tx, ty = map(int, line.split(":", 1)[1].split())
        self.xdo("key", "--window", self.window, "Escape", "End", "Return")
        self.xdo("key", "--window", self.window, "Home", "Down", "Down", "Down", "Return")
        browser = self.wait(lambda: self.changed(self.initial), "Puzzles band browser")
        self.xdo("key", "--window", self.window, "Return")
        self.wait(lambda: self.changed(browser), "puzzle setup position")
        before = self.wait(self.puzzle_ready, "opponent setup move presented")
        self.xdo("mousemove", "--window", self.window, fx, fy)
        self.xdo("click", 1)
        self.wait(lambda: difference(before, self.screenshot()) > 0.01,
                  "known puzzle solver selection")
        self.xdo("mousemove", "--window", self.window, tx, ty)
        self.xdo("click", 1)

        def solver_moved():
            image = self.screenshot()
            rects = [(x - 5, y - 5, x + 5, y + 5) for x, y in [(fx, fy), (tx, ty)]]
            return image if all(difference(before.crop(rect), image.crop(rect)) > 3
                                for rect in rects) else None

        solved_move = self.wait(solver_moved, "00008 e6-e7 accepted with scripted reply")
        solved_move.save(self.output / f"{self.name}-puzzle-first-move.png")
        self.records.append("Puzzles: 00008 first solver move by engine-projected clicks")

    def puzzle_ready(self):
        image = self.screenshot()
        # Retry turns from disabled grey to white after the setup transaction.
        center = self.width // 2 + 160
        retry = image.crop((center - 50, self.height - 52, center + 50, self.height - 20))
        bright = sum(min(pixel[:3]) > 235 for pixel in retry.getdata())
        return image if bright >= 8 else None

    def changed(self, reference_image):
        image = self.screenshot()
        return image if difference(reference_image, image) > 2 else None

    def camera_controls(self):
        self.xdo("mousemove", "--window", self.window, 400, 300)
        self.xdo("mousedown", 1)
        self.xdo("mousemove", "--window", self.window, 510, 350)
        self.xdo("mouseup", 1)
        dragged = self.wait(lambda: self.changed(self.initial), "drag changing chess pixels")
        dragged.save(self.output / f"{self.name}-dragged.png")
        self.xdo("key", "--window", self.window, "r")
        self.wait(lambda: self.matching(self.initial, 0), "R exact reset")
        self.xdo("click", 4)
        zoomed = self.wait(lambda: self.changed(self.initial), "wheel changing chess pixels")
        zoomed.save(self.output / f"{self.name}-zoomed.png")
        self.xdo("key", "--window", self.window, "Home")
        self.wait(lambda: self.matching(self.initial, 0), "Home exact reset")
        resized = self.resize_and_restore()
        self.records = ["drag changes chess pixels", "wheel changes chess pixels",
                        "R exact reset", "Home exact reset", "800x600 resize matches capture",
                        "unmap/map restoration"]
        self.xdo("windowfocus", self.window)
        self.xdo("key", "--window", self.window, "Escape")
        self.wait(lambda: difference(resized, self.screenshot()) > 2, "Escape opens game menu")
        self.xdo("key", "--window", self.window, "Escape")
        self.wait(lambda: self.matching(resized, 0), "Escape resumes board")
        self.records += ["Escape opens menu and resumes"]

    def resize_and_restore(self):
        resized_reference = self.output / f"{self.name}-resized-capture.png"
        self.progress("launch: headless 800x600 capture")
        subprocess.run([str(self.args.binary), "--capture", str(resized_reference),
                        "--view", self.view, "--width", "800", "--height", "600",
                        "--data-dir", self.data_dir], env=self.headless, check=True,
                       capture_output=True, timeout=CAPTURE_TIMEOUT_SECONDS)
        self.xdo("windowsize", self.window, 800, 600)
        resized = self.wait(lambda: self.matching(Image.open(resized_reference).convert("RGB")),
                            "resize matching 800x600 headless capture")
        resized.save(self.output / f"{self.name}-resized.png")
        self.xdo("windowunmap", self.window)
        self.wait(lambda: not ImageGrab.grab(xdisplay=DISPLAY).getbbox(), "unmap")
        self.xdo("windowmap", self.window)
        self.wait(lambda: self.matching(resized), "restored chess rendering")
        return resized

    def cleanup(self):
        if self.process.poll() is None:
            self.progress("cleanup: terminate chess window")
            self.process.terminate()
            try:
                self.wait_exit(10, "terminated chess window exit")
            except subprocess.TimeoutExpired:
                self.progress("cleanup: kill chess window")
                self.process.kill()
                self.wait_exit(5, "killed chess window exit")

    def finish(self):
        self.progress("close: chess window")
        close_request(self.window)
        self.records.append("WM_DELETE_WINDOW clean exit")
        assert self.wait_exit(15, "chess window exit") == 0
        assert self.saved.read_bytes() == self.saved_bytes, "camera controls changed position save"
        return {"case": self.name, "view": self.view, "saved_pieces": len(self.position["pieces"]),
                "rendered_pieces": 6 if self.view == "lineup" else len(self.position["pieces"]),
                "capture_window_mean_absolute_error": difference(self.expected, self.initial),
                "colors": len(self.colors), "controls": self.records, "exit": 0}

    def run(self):
        self.progress("launch: headless reference capture")
        self.expected = self.capture(self.output / f"{self.name}-capture.png")
        with (self.output / f"{self.name}.log").open("w") as log:
            opponent = ["--play", "white", "--level", "1"] if self.computer else []
            if not self.computer and not self.load and self.view != "lineup" and self.interaction != "title":
                opponent = ["--moves", ""]
            self.progress("launch: chess window")
            self.process = subprocess.Popen([str(self.args.binary), *self.common, *opponent,
                                            "--window", "--save", str(self.saved)],
                                           env=self.environment, stdout=log, stderr=log)
            try:
                self.find_window("chess window")
                self.initial_state()
                if self.view == "overhead" and not self.load:
                    self.play_and_undo(log)
                if self.interaction == "title":
                    self.puzzle_click()
                if self.interaction == "camera":
                    self.camera_controls()
                return self.finish()
            except Exception:
                self.progress("capture: failure screenshot")
                ImageGrab.grab(xdisplay=DISPLAY).save(self.output / f"{self.name}-failure.png")
                raise
            finally:
                self.cleanup()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--xdotool", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--case", dest="cases", action="append", choices=CASES,
                        help="run only this case (repeatable; default: all cases)")
    args = parser.parse_args()
    args.binary = args.binary.resolve()
    args.xdotool = args.xdotool.resolve()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, DISPLAY=DISPLAY, WINIT_X11_SCALE_FACTOR="1")
    for key in ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]:
        environment.pop(key, None)
    results = [run_case(args, environment, name, **CASES[name]) for name in (args.cases or CASES)]
    encoded = json.dumps(results, indent=2) + "\n"
    (args.output / "results.json").write_text(encoded)
    print(encoded, end="", flush=True)


if __name__ == "__main__":
    main()
