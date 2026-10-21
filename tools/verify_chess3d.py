#!/usr/bin/env python3
"""Real headless Chess3D acceptance. Python 3.10+, stdlib only; GPU absence fails.

Run --help for the portable interface. All generated evidence stays in --output;
Cargo artifacts stay in --target-dir. Existing nonempty output is never replaced.
"""
from __future__ import annotations

import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path
import re
import selectors
import signal
import stat
import subprocess
import sys
import time

from chess3d_image import compare, decode_png, image_stats, validate_image
from verify_rendering_boundary import verify

ROOT = Path(__file__).resolve().parents[1]
VIEWS = {"hero": (1400, 1050), "overhead": (1200, 1200), "lineup": (1400, 700)}
MAX_COMMAND_OUTPUT = 8 * 1024 * 1024
MAX_ICD_BYTES = 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def repository_info(path, *, cargo_checkout=False):
    """Record uncommitted migrations truthfully, including an unborn game HEAD."""
    def git(*args):
        return subprocess.run(["git", "-C", str(path), *args], capture_output=True,
                              text=True, timeout=10, check=False)

    revision = git("rev-parse", "--verify", "HEAD")
    status = git("status", "--porcelain")
    require(status.returncode == 0, f"cannot inspect checkout: {path}: {status.stderr}")
    receipt = {"path": str(path), "revision": revision.stdout.strip() if revision.returncode == 0 else None,
               "dirty": bool(status.stdout), "status": status.stdout.splitlines()}
    if cargo_checkout:
        top = git("rev-parse", "--show-toplevel")
        require(top.returncode == 0, "cannot locate Cargo SDK checkout root")
        marker = Path(top.stdout.strip()) / ".cargo-ok"
        # Cargo creates this empty untracked marker after a successful Git checkout.
        generated = [".cargo-ok"] if ("?? .cargo-ok" in receipt["status"] and
                                       marker.is_file() and not marker.is_symlink() and
                                       marker.stat().st_size == 0) else []
        receipt["cargo_generated_files"] = generated
        receipt["source_dirty"] = any(line != "?? .cargo-ok" or not generated
                                      for line in receipt["status"])
    return receipt


class Acceptance:
    def __init__(self, args):
        self.args = args
        require(os.name == "posix", "this headless Vulkan harness requires POSIX process groups")
        require(0 < args.timeout <= 900, "--timeout must be positive and at most 900 seconds")
        self.output = args.output.resolve()
        target = args.target_dir.resolve()
        require(not (target.is_relative_to(self.output) or self.output.is_relative_to(target)),
                "--target-dir and --output must not overlap")
        require(not ROOT.is_relative_to(target), "--target-dir cannot contain the repository")
        self.output.mkdir(parents=True, exist_ok=True)
        require(not any(self.output.iterdir()), "--output must be empty (previous evidence is preserved)")
        self.env = os.environ.copy()
        for key in ("DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "VK_ICD_FILENAMES", "MESA_VK_DEVICE_SELECT",
                    "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE", "WGPU_ADAPTER_NAME", "WGPU_BACKEND"):
            self.env.pop(key, None)
        require(bool(self.env.get("VK_DRIVER_FILES")), "set VK_DRIVER_FILES to one explicit Vulkan ICD JSON")
        icd = Path(self.env["VK_DRIVER_FILES"]).resolve()
        require(icd.is_file(), "VK_DRIVER_FILES must name one existing regular file (ICD JSON)")
        # Nonblocking open also prevents a replaced path from hanging on a FIFO.
        with os.fdopen(os.open(icd, os.O_RDONLY | os.O_NONBLOCK), "rb") as stream:
            metadata = os.fstat(stream.fileno())
            require(stat.S_ISREG(metadata.st_mode), "ICD input must be a regular file")
            require(metadata.st_size <= MAX_ICD_BYTES, "ICD file budget exceeded (1 MiB)")
            icd_data = stream.read(MAX_ICD_BYTES + 1)
        require(len(icd_data) <= MAX_ICD_BYTES, "ICD file budget exceeded (1 MiB)")
        self.env["VK_DRIVER_FILES"] = str(icd)
        self.env.update(CARGO_TARGET_DIR=str(args.target_dir.resolve()), CARGO_BUILD_JOBS="2",
                        CARGO_INCREMENTAL="0", LP_NUM_THREADS="2", CHESS3D_REQUIRE_GPU="1")
        self.report = {"schema": "chess3d-acceptance-v1", "status": "running",
                       "started_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                       "root": str(ROOT), "output": str(self.output),
                       "environment": {k: self.env.get(k) for k in
                                       ("VK_DRIVER_FILES", "DISPLAY", "WAYLAND_DISPLAY", "CARGO_TARGET_DIR", "CARGO_BUILD_JOBS", "LP_NUM_THREADS", "CHESS3D_REQUIRE_GPU")},
                       "icd_sha256": hashlib.sha256(icd_data).hexdigest(),
                       "commands": [], "images": {}, "comparisons": {}, "negative_controls": {}}
        suffix = ".exe" if os.name == "nt" else ""
        self.binary = args.target_dir.resolve() / "debug" / ("chess3d" + suffix)
        self.probe = args.target_dir.resolve() / "debug" / "examples" / ("acceptance_context" + suffix)
        self.images = {}
        self.fixture_dir = ROOT / "assets/positions"

    def command(self, name, command, *, env=None, success=True, diagnostic=None):
        require(re.fullmatch(r"[a-z0-9-]+", name), "invalid command name")
        start = time.monotonic()
        deadline = start + self.args.timeout
        command = [str(c) for c in command]
        log = self.output / f"{name}.log"
        receipt = {"name": name, "argv": command, "returncode": None,
                   "vk_driver_files": (env or self.env).get("VK_DRIVER_FILES"), "log": str(log)}
        self.report["commands"].append(receipt)
        output = bytearray()
        process = None
        try:
            with log.open("xb") as stream, selectors.DefaultSelector() as selector:
                stream.write(f"$ {command!r}\n\ncombined stdout/stderr:\n".encode())
                process = subprocess.Popen(command, cwd=ROOT, env=env or self.env,
                                           stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
                                           stdin=subprocess.DEVNULL, start_new_session=True)
                with process.stdout:
                    selector.register(process.stdout, selectors.EVENT_READ)
                    while selector.get_map():
                        remaining = deadline - time.monotonic()
                        require(remaining > 0, f"subprocess deadline exceeded: {log}")
                        for key, _ in selector.select(remaining):
                            block = os.read(key.fd, 65536)
                            if not block:
                                selector.unregister(key.fileobj)
                                continue
                            available = MAX_COMMAND_OUTPUT - len(output)
                            stream.write(block[:available])
                            output.extend(block[:available])
                            require(len(block) <= available, f"subprocess output budget exceeded: {log}")
                    try:
                        process.wait(timeout=max(0.001, deadline - time.monotonic()))
                    except subprocess.TimeoutExpired as error:
                        raise ValueError(f"subprocess deadline exceeded: {log}") from error
            text = output.decode("utf-8", errors="replace")
            require((process.returncode == 0) == success, f"unexpected exit {process.returncode}: {log}")
            if diagnostic:
                require(diagnostic.lower() in text.lower(), f"missing diagnostic {diagnostic!r}: {log}")
            return text
        except (ValueError, OSError) as error:
            receipt["error"] = str(error)
            raise
        finally:
            if process is not None:
                # Kill the whole command tree, including children retaining a pipe
                # after the direct child exits; never leave timed-out Cargo/GPU work.
                try:
                    os.killpg(process.pid, signal.SIGKILL)
                except ProcessLookupError:
                    pass
                process.wait(timeout=5)
                receipt["returncode"] = process.returncode
            receipt["seconds"] = time.monotonic() - start

    def capture(self, name, *, view, extra=()):
        path = self.output / f"{name}.png"
        width, height = VIEWS[view]
        self.command(name, [self.binary, "--view", view, "--width", width, "--height", height,
                            "--capture", path, *extra])
        image = decode_png(path)
        stats = image_stats(image)
        stats["gates"] = validate_image(stats, (width, height))
        stats["path"] = str(path)
        self.report["images"][name] = stats
        self.images[name] = image
        return image

    def comparison(self, name, a, b, identical):
        measurement = compare(self.images[a], self.images[b])
        self.report["comparisons"][name] = {"a": a, "b": b, **measurement}
        require((measurement["changed_pixels"] == 0) == identical,
                f"{name}: expected {'identical' if identical else 'different'} decoded pixels: {measurement}")

    def negatives(self):
        invalid = self.output / "invalid-position.json"
        invalid.write_text('{"format":"chess3d-position","version":1,"seed":342,"style":"nocturne-ivory",'
                           '"pieces":[{"id":"bad","side":"ivory","kind":"king","square":{"file":8,"rank":0}}]}', encoding="utf-8")
        for name, args, diagnostic in [
            ("invalid-position", ["--load", invalid], "InvalidSquare"),
            ("invalid-width", ["--width", "0"], "dimensions must be"),
            ("invalid-height", ["--height", "4097"], "dimensions must be"),
            ("no-adapter", [], "NoAdapter"),
        ]:
            capture = self.output / f"protected-{name}.png"
            save = self.output / f"protected-{name}.json"
            original = self.images["hero"][2][:128]  # Distinct sentinel; not a fabricated capture.
            capture.write_bytes(original)
            save.write_bytes((self.output / "initial-saved.json").read_bytes())
            before = {p: p.read_bytes() for p in (capture, save, invalid)}
            env = self.env.copy()
            if name == "no-adapter":
                env["VK_DRIVER_FILES"] = str(self.output / "deliberately-missing-icd.json")
            # Save succeeds before GPU initialization by current CLI contract; the
            # same authoritative initial seed preserves its exact preexisting JSON.
            self.command(name, [self.binary, "--capture", capture, "--save", save, *args],
                         env=env, success=False, diagnostic=diagnostic)
            require(all(p.read_bytes() == data for p, data in before.items()), f"{name} damaged existing files")
            fresh = self.output / f"absent-{name}.png"
            self.command(name + "-absent", [self.binary, "--capture", fresh, *args],
                         env=env, success=False, diagnostic=diagnostic)
            require(not fresh.exists(), f"{name} created a capture after failure")
            self.report["negative_controls"][name] = {"existing_capture_preserved": True,
                                                      "existing_save_preserved": True,
                                                      "input_preserved": True, "absent_capture_not_created": True}

    def baseline(self):
        if not self.args.baseline_dir:
            self.report["baseline"] = {"status": "not supplied; no art-improvement claim"}
            return
        baseline = {}
        for name in ("hero", "overhead", "lineup", "study"):
            path = self.args.baseline_dir.resolve() / f"chess3d-{name}.png"
            old = decode_png(path)
            baseline[name] = {"path": str(path), "stats": image_stats(old),
                              "comparison": compare(old, self.images[name])}
        self.report["baseline"] = {"status": "measured, informational; no cross-GPU equality gate", "images": baseline}

    def parity(self):
        width, height, pixels = self.images["overhead"]
        samples = []
        groups = {True: [], False: []}
        for center in self.report["context"]["overhead_centers"]:
            x, y = (round(v) for v in center["pixel"])
            require(1 <= x < width - 1 and 1 <= y < height - 1, "projected center outside image")
            # Empty middle ranks avoid confusing a sculpt's silhouette with a tile.
            colors = [pixels[((y + dy) * width + x + dx) * 4:((y + dy) * width + x + dx) * 4 + 3]
                      for dy in (-1, 0, 1) for dx in (-1, 0, 1)]
            luma = sum(sum(c * w for c, w in zip(rgb, (0.2126, 0.7152, 0.0722))) for rgb in colors) / len(colors)
            groups[center["dark"]].append(luma)
            samples.append({**center, "luma": luma})
        # A broad non-overlap gate detects mirrored/shifted parity, inverted UVs,
        # and missing board texture without pinning specific GPU channel values.
        require(max(groups[True]) < min(groups[False]), "projected dark/light tile centers overlap")
        self.report["overhead_parity"] = {"samples": samples, "dark_max": max(groups[True]),
                                           "light_min": min(groups[False]), "passed": True}

    def run(self):
        cargo = ["cargo", "+1.97.0"]
        self.report["game_checkout"] = repository_info(ROOT)
        self.report["rendering_boundary"] = verify(require_canonical=True)
        facade = Path(self.report["rendering_boundary"]["graphs"][0]["sdk_manifest"])
        self.report["engine_checkout"] = repository_info(facade.parent, cargo_checkout=True)
        require(self.report["engine_checkout"]["revision"] ==
                self.report["rendering_boundary"]["graphs"][0]["sdk_revision"],
                "actual Cargo SDK checkout HEAD does not match immutable source")
        require(not self.report["engine_checkout"]["source_dirty"], "actual Cargo SDK checkout is dirty")
        self.report["rustc"] = self.command("rustc", ["rustc", "+1.97.0", "--version"]).strip()
        self.command("game-tests", [*cargo, "test", "--locked", "-j2", "-p", "chess3d", "--", "--test-threads=2", "--nocapture"])
        self.command("build", [*cargo, "build", "--locked", "-j2", "-p", "chess3d", "--bin", "chess3d", "--example", "acceptance_context"])
        with self.binary.open("rb") as binary:
            digest = hashlib.sha256()
            for block in iter(lambda: binary.read(1024 * 1024), b""):
                digest.update(block)
        self.report["binary_sha256"] = digest.hexdigest()
        self.report["context"] = json.loads(self.command("context-before", [self.probe]))
        require(self.report["context"]["adapter"]["backend"] == "Vulkan", "explicit ICD acceptance requires Vulkan")
        saved = self.output / "initial-saved.json"
        self.capture("hero", view="hero", extra=("--seed", "342", "--save", saved))
        self.capture("hero-repeat", view="hero", extra=("--seed", "342"))
        self.comparison("hero_repeat", "hero", "hero-repeat", True)
        self.capture("initial-reloaded", view="hero", extra=("--load", saved))
        self.comparison("initial_roundtrip", "hero", "initial-reloaded", True)
        require(json.loads(saved.read_text()) == json.loads(
            (self.fixture_dir / "initial.json").read_text()),
            "saved initial document differs from the authoritative initial fixture")
        for view in ("overhead", "lineup"):
            self.capture(view, view=view)
            self.capture(view + "-repeat", view=view)
            self.comparison(view + "_repeat", view, view + "-repeat", True)
        study = self.fixture_dir / "study.json"
        study_saved = self.output / "study-saved.json"
        self.capture("study", view="hero", extra=("--load", study, "--save", study_saved))
        require(json.loads(study.read_text()) == json.loads(study_saved.read_text()), "study semantic roundtrip changed seed/current squares")
        self.capture("study-reloaded", view="hero", extra=("--load", study_saved))
        self.comparison("study_roundtrip", "study", "study-reloaded", True)
        self.comparison("study_differs_initial", "hero", "study", False)
        # Study already tests live squares; also test capture/removal and a moved rook.
        edited = json.loads(study_saved.read_text())
        edited["pieces"] = [p for p in edited["pieces"] if p["square"] != {"file": 0, "rank": 3}]
        edited["pieces"][0]["square"] = {"file": 0, "rank": 3}
        edited["pieces"].pop()
        edited_path, edited_saved = self.output / "edited.json", self.output / "edited-saved.json"
        edited_path.write_text(json.dumps(edited), encoding="utf-8")
        self.capture("edited", view="hero", extra=("--load", edited_path, "--save", edited_saved))
        require(json.loads(edited_saved.read_text()) == edited, "edited semantic roundtrip changed")
        self.capture("edited-reloaded", view="hero", extra=("--load", edited_saved))
        self.comparison("edited_roundtrip", "edited", "edited-reloaded", True)
        self.comparison("edited_differs_study", "study", "edited", False)
        self.capture("changed-seed", view="hero", extra=("--seed", str(342 + (1 << 48))))
        self.comparison("seed_changes_texture", "hero", "changed-seed", False)
        self.parity()
        self.negatives()
        no_adapter = self.env.copy()
        no_adapter["VK_DRIVER_FILES"] = str(self.output / "deliberately-missing-icd.json")
        self.command("required-gpu-no-adapter", [*cargo, "test", "--locked", "-j2", "-p", "chess3d",
                                                "--test", "gpu_acceptance", "--", "--test-threads=1"],
                     env=no_adapter, success=False, diagnostic="requires a real GPU device")
        self.report["negative_controls"]["required_gpu_test"] = {"missing_adapter_fails": True}
        self.baseline()
        after = json.loads(self.command("context-after", [self.probe]))
        require(after["adapter"] == self.report["context"]["adapter"], "adapter changed during acceptance")
        self.report["context_after"] = after
        self.report["status"] = "passed"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path, help="new/empty directory for JSON, logs, PNGs and positions")
    parser.add_argument("--target-dir", type=Path, default=Path(os.environ.get("CARGO_TARGET_DIR", ROOT / "target")))
    parser.add_argument("--baseline-dir", type=Path, help="optional genuine chess3d-{hero,overhead,lineup,study}.png directory")
    parser.add_argument("--timeout", type=int, default=600, help="per-subprocess timeout in seconds")
    args = parser.parse_args()
    acceptance = None
    try:
        acceptance = Acceptance(args)
        acceptance.run()
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        if acceptance:
            acceptance.report.update(status="failed", error=str(error))
        print(f"Chess3D acceptance FAILED: {error}", file=sys.stderr)
        return 1
    finally:
        if acceptance:
            report = acceptance.output / "report.json"
            report.write_text(json.dumps(acceptance.report, indent=2) + "\n", encoding="utf-8")
            print(f"Acceptance report: {report}")
    print("Chess3D acceptance passed (real CLI captures; no GPU skips)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
