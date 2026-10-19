#!/usr/bin/env python3
"""Run the original real-window controls harness on isolated Xvfb :100.

Missing window/GPU prerequisites fail, never skip. Does not install packages or
use the desktop display. Keep :100 exclusive for this bounded smoke test.
"""
from __future__ import annotations

import argparse
import importlib.util
import os
from pathlib import Path
import shutil
import signal
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]


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


def run(args):
    tools, binary, icd = prerequisites(args.target_dir.resolve())
    output = args.output.resolve()
    if output.exists() and any(output.iterdir()):
        raise ValueError("--output must be empty; previous window evidence is preserved")
    environment = dict(os.environ, VK_DRIVER_FILES=str(icd), CHESS3D_REQUIRE_GPU="1", LP_NUM_THREADS="2")
    for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "VK_ICD_FILENAMES",
                "WGPU_ADAPTER_NAME", "WGPU_BACKEND", "MESA_VK_DEVICE_SELECT",
                "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE"):
        environment.pop(key, None)
    command = [tools["xvfb-run"], "--server-num=100", "--server-args=-screen 0 1600x1200x24 -nolisten tcp",
               sys.executable, str(ROOT / "tests/window_smoke.py"), "--binary", str(binary),
               "--xdotool", tools["xdotool"], "--output", str(output)]
    process = subprocess.Popen(command, cwd=ROOT, env=environment, start_new_session=True)
    try:
        status = process.wait(timeout=300)
        if status:
            raise ValueError(f"real-window harness failed with exit {status}")
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
