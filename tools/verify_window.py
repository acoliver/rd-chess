#!/usr/bin/env python3
"""Run the original real-window controls harness on an isolated Xvfb display.

Missing window/GPU prerequisites fail, never skip. Does not install packages or
use the desktop display. Pick a free server number starting at :100.
"""
from __future__ import annotations

import argparse
from collections import deque
import importlib.util
import json
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CASES = ("hero", "overhead", "title-to-friend", "computer-level-1", "study", "lineup")
CASE_TIMEOUT = 150


def prerequisites(target):
    tools = {}
    for tool in ("xvfb-run", "Xvfb", "xauth", "xdotool"):
        executable = shutil.which(tool)
        if not executable:
            raise ValueError(f"required window prerequisite missing: {tool}; window acceptance NOT verified")
        tools[tool] = executable
    if importlib.util.find_spec("PIL") is None:
        raise ValueError("required window prerequisite missing: Pillow")
    icd = Path(os.environ.get("VK_DRIVER_FILES", ""))
    if not icd.is_file():
        raise ValueError("set VK_DRIVER_FILES to one explicit existing Vulkan ICD JSON")
    binary = target / "debug/chess3d"
    if not binary.is_file():
        raise ValueError("viewer binary missing; first run cargo xtask build")
    return tools, binary.resolve(), icd.resolve()


def log_tail(path, count, prefix=""):
    if not path.is_file():
        return "(log not created)\n"
    with path.open(errors="replace") as stream:
        lines = (line for line in stream if line.startswith(prefix))
        return "".join(deque(lines, maxlen=count)) or "(no matching log lines)\n"


def run_case(name, output, tools, binary, environment):
    output.mkdir()
    xvfb_log = output / "xvfb.log"
    progress_log = output / "smoke.log"
    command = [tools["xvfb-run"], "--auto-servernum", "--server-num=100", f"--error-file={xvfb_log}",
               "--server-args=-screen 0 1600x1200x24 -nolisten tcp",
               sys.executable, str(ROOT / "tests/window_smoke.py"), "--binary", str(binary),
               "--xdotool", tools["xdotool"], "--output", str(output), "--case", name]
    failure = None
    print(f"Window case {name}: starting (limit {CASE_TIMEOUT}s)", flush=True)
    with progress_log.open("w") as stream:
        process = subprocess.Popen(command, cwd=ROOT, env=environment, start_new_session=True,
                                   stdout=stream, stderr=subprocess.STDOUT)
        try:
            try:
                status = process.wait(timeout=CASE_TIMEOUT)
            except subprocess.TimeoutExpired:
                failure = f"timed out after {CASE_TIMEOUT} seconds"
            else:
                if status:
                    failure = f"failed with exit {status}"
        finally:
            try:
                os.killpg(process.pid, signal.SIGTERM)
            except ProcessLookupError:
                pass
            try:
                process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                os.killpg(process.pid, signal.SIGKILL)
                process.wait(timeout=5)
    if failure:
        raise ValueError(
            f"real-window harness case {name} {failure}\n"
            f"case {name}: {progress_log} (last 40 progress lines):\n"
            f"{log_tail(progress_log, 40, '[window case=')}"
            f"case {name}: {progress_log} (last 20 output lines):\n{log_tail(progress_log, 20)}"
            f"case {name}: {xvfb_log} (last 20 lines):\n{log_tail(xvfb_log, 20)}")
    results = json.loads((output / "results.json").read_text())
    if len(results) != 1 or results[0]["case"] != name or results[0]["exit"] != 0:
        raise ValueError(f"real-window harness case {name} returned unexpected results")
    print(f"Window case {name}: passed ({results[0]['elapsed_seconds']:.2f}s)", flush=True)
    return results[0]


def run(args):
    tools, binary, icd = prerequisites(args.target_dir.resolve())
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        raise ValueError("--output must be empty; previous window evidence is preserved")
    output.mkdir(parents=True, exist_ok=True)
    environment = dict(os.environ, VK_DRIVER_FILES=str(icd), CHESS3D_REQUIRE_GPU="1", LP_NUM_THREADS="2")
    for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "VK_ICD_FILENAMES",
                "WGPU_ADAPTER_NAME", "WGPU_BACKEND", "MESA_VK_DEVICE_SELECT",
                "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE"):
        environment.pop(key, None)
    results = []
    for name in CASES:
        results.append(run_case(name, output / name, tools, binary, environment))
        (output / "results.json").write_text(json.dumps(results, indent=2) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--target-dir", type=Path,
                        default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")))
    args = parser.parse_args()
    try:
        run(args)
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        print(f"Window acceptance FAILED: {error}", file=sys.stderr)
        return 1
    print(f"Window acceptance passed: {args.output.resolve() / 'results.json'}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
