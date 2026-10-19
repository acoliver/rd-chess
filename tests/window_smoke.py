#!/usr/bin/env python3
"""Opt-in real Chess3D pixel/input smoke on an exclusive Xvfb :100 (never desktop).

Requires Pillow, xdotool, libX11 and a chess3d binary built with --features viewer.
The caller starts Xvfb :100 and chooses Vulkan ICD/library paths. Each window is
selected by child PID. Capture references exercise the same real game scene.
"""

import argparse
import ctypes as c
import json
import os
from pathlib import Path
import subprocess
import time

from PIL import Image, ImageChops, ImageGrab, ImageStat

DISPLAY = ":100"


def wait_for(predicate, label):
    deadline = time.monotonic() + 30
    while time.monotonic() < deadline:
        result = predicate()
        if result:
            return result
        time.sleep(0.05)
    raise AssertionError(f"Timed out waiting for {label}")


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


def run_case(args, environment, name, view, load=None, controls=False):
    output = args.output
    saved = output / f"{name}-position.json"
    reference = output / f"{name}-capture.png"
    width, height = (1200, 600) if view == "lineup" else (960, 720)
    common = ["--view", view, "--width", str(width), "--height", str(height)]
    if load:
        common += ["--load", str(load)]
    headless = dict(environment)
    for key in ["DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET"]:
        headless.pop(key, None)
    subprocess.run([str(args.binary), *common, "--capture", str(reference)],
                   env=headless, check=True, capture_output=True, timeout=60)
    expected = Image.open(reference).convert("RGB")

    def xdo(*arguments, check=True):
        return subprocess.run([str(args.xdotool), *map(str, arguments)],
                              env=environment, capture_output=True, text=True,
                              check=check, timeout=10).stdout.strip()

    with (output / f"{name}.log").open("w") as log:
        process = subprocess.Popen([str(args.binary), *common, "--window", "--save", str(saved)],
                                   env=environment, stdout=log, stderr=log)
        try:
            window = wait_for(lambda: xdo("search", "--pid", process.pid, "--name",
                                         "^Chess3D", check=False), "chess window")
            xdo("windowmove", window, 0, 0)
            xdo("windowfocus", window)

            def screenshot():
                fields = dict(line.split("=", 1) for line in
                              xdo("getwindowgeometry", "--shell", window).splitlines())
                x, y, width, height = [int(fields[key]) for key in ["X", "Y", "WIDTH", "HEIGHT"]]
                return ImageGrab.grab(bbox=(x, y, x + width, y + height), xdisplay=DISPLAY)

            def matching(reference_image, tolerance=0.1):
                image = screenshot()
                return image if difference(reference_image, image) <= tolerance else None

            initial = wait_for(lambda: matching(expected), "window matching headless chess capture")
            initial.save(output / f"{name}-initial.png")
            colors = initial.getcolors(2_000_000)
            assert colors and len(colors) > 1000, "missing textured chess content"
            # The wide lineup has large empty studio margins; check its subject band.
            subject = initial.crop((0, height // 3, width, 3 * height // 4)) if view == "lineup" else initial
            assert min(ImageStat.Stat(subject).stddev) > 25, "blank/flat chess subject"
            position = json.loads(saved.read_text())
            if load:
                assert position == json.loads(load.read_text()), "loaded position changed"
            else:
                assert len(position["pieces"]) == 32
                assert position["seed"] == 342
            saved_bytes = saved.read_bytes()
            records = []
            if controls:
                def changed(reference_image):
                    image = screenshot()
                    return image if difference(reference_image, image) > 2 else None

                xdo("mousemove", "--window", window, 400, 300)
                xdo("mousedown", 1)
                xdo("mousemove", "--window", window, 510, 350)
                xdo("mouseup", 1)
                dragged = wait_for(lambda: changed(initial), "drag changing chess pixels")
                dragged.save(output / f"{name}-dragged.png")
                xdo("key", "--window", window, "r")
                wait_for(lambda: matching(initial, 0), "R exact reset")
                xdo("click", 4)
                zoomed = wait_for(lambda: changed(initial), "wheel changing chess pixels")
                zoomed.save(output / f"{name}-zoomed.png")
                xdo("key", "--window", window, "Home")
                wait_for(lambda: matching(initial, 0), "Home exact reset")

                resized_reference = output / f"{name}-resized-capture.png"
                subprocess.run([str(args.binary), "--capture", str(resized_reference),
                                "--view", view, "--width", "800", "--height", "600"],
                               env=headless, check=True, capture_output=True, timeout=60)
                xdo("windowsize", window, 800, 600)
                resized = wait_for(lambda: matching(Image.open(resized_reference).convert("RGB")),
                                   "resize matching 800x600 headless capture")
                resized.save(output / f"{name}-resized.png")
                xdo("windowunmap", window)
                wait_for(lambda: not ImageGrab.grab(xdisplay=DISPLAY).getbbox(), "unmap")
                xdo("windowmap", window)
                wait_for(lambda: matching(resized), "restored chess rendering")
                records = ["drag changes chess pixels", "wheel changes chess pixels",
                           "R exact reset", "Home exact reset", "800x600 resize matches capture",
                           "unmap/map restoration"]
                xdo("windowfocus", window)
                xdo("key", "--window", window, "Escape")
                records.append("Escape clean exit")
            else:
                close_request(window)
                records.append("WM_DELETE_WINDOW clean exit")
            assert process.wait(timeout=15) == 0
            assert saved.read_bytes() == saved_bytes, "camera controls changed position save"
            return {"case": name, "view": view, "saved_pieces": len(position["pieces"]),
                    "rendered_pieces": 6 if view == "lineup" else len(position["pieces"]),
                    "capture_window_mean_absolute_error": difference(expected, initial),
                    "colors": len(colors), "controls": records, "exit": 0}
        except Exception:
            ImageGrab.grab(xdisplay=DISPLAY).save(output / f"{name}-failure.png")
            raise
        finally:
            if process.poll() is None:
                process.terminate()
                process.wait(timeout=10)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--xdotool", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    args.binary = args.binary.resolve()
    args.xdotool = args.xdotool.resolve()
    args.output = args.output.resolve()
    args.output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, DISPLAY=DISPLAY, WINIT_X11_SCALE_FACTOR="1")
    for key in ["WAYLAND_DISPLAY", "WAYLAND_SOCKET"]:
        environment.pop(key, None)
    fixture = Path(__file__).resolve().parent.parent / "assets/positions/study.json"
    results = [run_case(args, environment, "hero", "hero", controls=True),
               run_case(args, environment, "overhead", "overhead"),
               run_case(args, environment, "study", "hero", load=fixture),
               run_case(args, environment, "lineup", "lineup")]
    encoded = json.dumps(results, indent=2) + "\n"
    (args.output / "results.json").write_text(encoded)
    print(encoded, end="")


if __name__ == "__main__":
    main()
