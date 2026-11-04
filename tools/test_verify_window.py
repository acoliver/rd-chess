#!/usr/bin/env python3
"""Window wrapper controls with fake tools and subprocesses; no Xvfb or GPU needed."""
import importlib.util
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, call, patch

import verify_window


def load_smoke():
    spec = importlib.util.spec_from_file_location("window_smoke", verify_window.ROOT / "tests/window_smoke.py")
    module = importlib.util.module_from_spec(spec)
    with patch.dict(os.environ, {"DISPLAY": ":107"}), patch.dict(sys.modules, {"PIL": Mock()}):
        spec.loader.exec_module(module)
    return module


def case_result(name):
    return {"case": name, "view": "hero", "saved_pieces": 32, "rendered_pieces": 32,
            "capture_window_mean_absolute_error": 0.02, "colors": 1001,
            "controls": ["WM_DELETE_WINDOW clean exit"], "exit": 0, "elapsed_seconds": 1.25}


class WindowHarnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.args = SimpleNamespace(output=self.root / "evidence", target_dir=self.root / "target")
        self.process = Mock(pid=12345)
        self.process.wait.return_value = 0
        prerequisites = patch("verify_window.prerequisites", return_value=(
            {"xvfb-run": "/fake/xvfb-run", "xdotool": "/fake/xdotool"},
            self.args.target_dir / "debug/chess3d", self.root / "icd.json"))
        prerequisites.start()
        self.addCleanup(prerequisites.stop)
        popen = patch("verify_window.subprocess.Popen", side_effect=self.launch)
        self.popen = popen.start()
        self.addCleanup(popen.stop)
        killpg = patch("verify_window.os.killpg")
        self.killpg = killpg.start()
        self.addCleanup(killpg.stop)

    def launch(self, command, **kwargs):
        name = command[command.index("--case") + 1]
        output = Path(command[command.index("--output") + 1])
        (output / "results.json").write_text(json.dumps([case_result(name)]))
        kwargs["stdout"].write("\n".join(
            f"[window case={name} elapsed={index}s] wait: step {index}" for index in range(50)) + "\n")
        return self.process

    def test_each_case_selects_free_display_and_has_its_own_limit_and_logs(self):
        verify_window.run(self.args)
        self.assertEqual(self.popen.call_count, len(verify_window.CASES))
        for name, invocation in zip(verify_window.CASES, self.popen.call_args_list):
            command = invocation.args[0]
            self.assertIn("--auto-servernum", command)
            self.assertIn("--server-num=100", command)
            self.assertIn(f"--error-file={self.args.output / name / 'xvfb.log'}", command)
            self.assertEqual(command[command.index("--case") + 1], name)
            self.assertEqual(command[command.index("--output") + 1], str(self.args.output / name))
            self.assertEqual(invocation.kwargs["stdout"].name, str(self.args.output / name / "smoke.log"))
            self.assertEqual(invocation.kwargs["stderr"], subprocess.STDOUT)
            self.assertTrue(invocation.kwargs["start_new_session"])
        self.assertEqual(self.process.wait.call_args_list,
                         [call(timeout=150), call(timeout=5)] * len(verify_window.CASES))

    def test_merges_complete_case_results_in_original_order(self):
        verify_window.run(self.args)
        results = json.loads((self.args.output / "results.json").read_text())
        self.assertEqual(results, [case_result(name) for name in verify_window.CASES])
        for name, result in zip(verify_window.CASES, results):
            self.assertEqual(json.loads((self.args.output / name / "results.json").read_text()), [result])

    def test_failure_reports_case_progress_and_xvfb_log_tails(self):
        self.process.wait.return_value = 1

        def launch(command, **kwargs):
            self.launch(command, **kwargs)
            kwargs["stdout"].write("\n".join(f"traceback line {index}" for index in range(60)) + "\n")
            lines = ["old diagnostic"] + [f"Xvfb diagnostic {index}" for index in range(20)]
            (self.args.output / "hero/xvfb.log").write_text("\n".join(lines) + "\n")
            return self.process

        self.popen.side_effect = launch
        with self.assertRaises(ValueError) as raised:
            verify_window.run(self.args)
        message = str(raised.exception)
        self.assertIn("real-window harness case hero failed with exit 1", message)
        self.assertIn(str(self.args.output / "hero/xvfb.log"), message)
        self.assertIn("Xvfb diagnostic 19", message)
        self.assertNotIn("old diagnostic", message)
        self.assertIn("[window case=hero elapsed=10s] wait: step 10", message)
        self.assertIn("[window case=hero elapsed=49s] wait: step 49", message)
        self.assertNotIn("wait: step 9\n", message)
        self.assertIn("traceback line 59", message)
        self.assertNotIn("traceback line 39\n", message)
        self.assertEqual(self.popen.call_count, 1)

    def test_timeout_reports_case_limit_progress_tail_and_preserves_completed_results(self):
        self.process.wait.side_effect = [0, 0, 0, 0, subprocess.TimeoutExpired("xvfb-run", 150), 0]

        def launch(command, **kwargs):
            self.launch(command, **kwargs)
            output = Path(command[command.index("--output") + 1])
            (output / "xvfb.log").write_text("Xvfb still running\n")
            return self.process

        self.popen.side_effect = launch
        with self.assertRaises(ValueError) as raised:
            verify_window.run(self.args)
        message = str(raised.exception)
        self.assertIn("case title-to-friend timed out after 150 seconds", message)
        self.assertIn("Xvfb still running", message)
        self.assertIn("[window case=title-to-friend elapsed=10s] wait: step 10", message)
        self.assertIn("[window case=title-to-friend elapsed=49s] wait: step 49", message)
        self.assertNotIn("wait: step 9\n", message)
        self.assertIn("case title-to-friend:", message)
        self.assertIn("title-to-friend/xvfb.log (last 20 lines)", message)
        self.assertEqual(self.popen.call_count, 3)
        self.assertEqual(json.loads((self.args.output / "results.json").read_text()),
                         [case_result(name) for name in verify_window.CASES[:2]])
        self.killpg.assert_called_with(self.process.pid, signal.SIGTERM)

    def test_timeout_kills_group_if_termination_does_not_finish(self):
        self.process.wait.side_effect = [subprocess.TimeoutExpired("xvfb-run", 150),
                                         subprocess.TimeoutExpired("xvfb-run", 5), 0]
        with self.assertRaisesRegex(ValueError, "case hero timed out after 150 seconds"):
            verify_window.run(self.args)
        self.assertEqual(self.killpg.call_args_list,
                         [call(self.process.pid, signal.SIGTERM), call(self.process.pid, signal.SIGKILL)])
        self.assertEqual(self.process.wait.call_args_list, [call(timeout=150), call(timeout=5), call(timeout=5)])

    def test_failure_without_xvfb_log_still_reports_exit(self):
        self.process.wait.return_value = 7
        with self.assertRaisesRegex(ValueError, "case hero failed with exit 7") as raised:
            verify_window.run(self.args)
        self.assertIn("(log not created)", str(raised.exception))

    def test_success_with_wrong_case_result_is_rejected(self):
        def launch(command, **kwargs):
            self.launch(command, **kwargs)
            (self.args.output / "hero/results.json").write_text(json.dumps([case_result("lineup")]))
            return self.process

        self.popen.side_effect = launch
        with self.assertRaisesRegex(ValueError, "case hero returned unexpected results"):
            verify_window.run(self.args)
        self.assertFalse((self.args.output / "results.json").exists())

    def test_main_prints_timeout_diagnostics_and_fails(self):
        self.process.wait.side_effect = [subprocess.TimeoutExpired("xvfb-run", 150), 0]
        with patch.object(sys, "argv", ["verify_window.py", "--output", str(self.args.output)]), \
                patch("sys.stderr", new_callable=io.StringIO) as stderr:
            self.assertEqual(verify_window.main(), 1)
        self.assertIn("Window acceptance FAILED:", stderr.getvalue())
        self.assertIn("case hero timed out after 150 seconds", stderr.getvalue())
        self.assertIn("wait: step 49", stderr.getvalue())

    def test_nonempty_output_is_preserved(self):
        self.args.output.mkdir()
        sentinel = self.args.output / "xvfb.log"
        sentinel.write_text("previous evidence")
        with self.assertRaisesRegex(ValueError, "--output must be empty"):
            verify_window.run(self.args)
        self.assertEqual(sentinel.read_text(), "previous evidence")
        self.popen.assert_not_called()

    def test_wrapper_does_not_forward_desktop_display_or_gpu_overrides(self):
        overrides = {key: "desktop" for key in (
            "DISPLAY", "WAYLAND_DISPLAY", "WAYLAND_SOCKET", "VK_ICD_FILENAMES",
            "WGPU_ADAPTER_NAME", "WGPU_BACKEND", "MESA_VK_DEVICE_SELECT",
            "MESA_VK_DEVICE_SELECT_FORCE_DEFAULT_DEVICE")}
        with patch.dict(os.environ, overrides):
            verify_window.run(self.args)
        for invocation in self.popen.call_args_list:
            environment = invocation.kwargs["env"]
            for key in overrides:
                self.assertNotIn(key, environment)
            self.assertEqual(environment["LP_NUM_THREADS"], "2")
            self.assertEqual(environment["CHESS3D_REQUIRE_GPU"], "1")
            self.assertEqual(environment["VK_DRIVER_FILES"], str(self.root / "icd.json"))

    def test_child_uses_display_selected_by_xvfb_run(self):
        self.assertEqual(load_smoke().DISPLAY, ":107")


class SmokeControlsTests(unittest.TestCase):
    def setUp(self):
        self.smoke = load_smoke()
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.args = SimpleNamespace(binary=self.root / "chess3d", xdotool=self.root / "xdotool",
                                    output=self.root)

    def main(self, *cases):
        command = ["window_smoke.py", "--binary", str(self.args.binary),
                   "--xdotool", str(self.args.xdotool), "--output", str(self.root)]
        for name in cases:
            command += ["--case", name]
        with patch.object(sys, "argv", command), patch.object(self.smoke, "run_case") as run_case, \
                patch("builtins.print"):
            run_case.side_effect = lambda args, env, name, **kwargs: case_result(name)
            self.smoke.main()
        return run_case

    def test_default_case_list_matches_wrapper(self):
        self.assertEqual(tuple(self.smoke.CASES), verify_window.CASES)
        run_case = self.main()
        self.assertEqual([invocation.args[2] for invocation in run_case.call_args_list],
                         list(verify_window.CASES))
        study = run_case.call_args_list[4].kwargs
        self.assertEqual(study["load"], verify_window.ROOT / "assets/positions/study.json")

    def test_single_and_repeated_case_selection(self):
        for selected in [("title-to-friend",), ("lineup", "hero")]:
            with self.subTest(selected=selected):
                run_case = self.main(*selected)
                self.assertEqual([invocation.args[2] for invocation in run_case.call_args_list], list(selected))
                self.assertEqual(json.loads((self.root / "results.json").read_text()),
                                 [case_result(name) for name in selected])

    def test_unknown_case_fails_before_launch(self):
        with patch.object(sys, "argv", ["window_smoke.py", "--binary", "fake", "--xdotool", "fake",
                                        "--output", str(self.root), "--case", "unknown"]), \
                patch("sys.stderr", new_callable=io.StringIO), \
                patch.object(self.smoke, "run_case") as run_case:
            with self.assertRaises(SystemExit) as raised:
                self.smoke.main()
        self.assertEqual(raised.exception.code, 2)
        run_case.assert_not_called()

    def test_case_elapsed_and_flushed_start_end_progress(self):
        with patch.object(self.smoke, "run_isolated_case", return_value={"case": "hero"}), \
                patch.object(self.smoke.time, "monotonic", side_effect=[10, 11, 12, 13]), \
                patch("builtins.print") as output:
            result = self.smoke.run_case(self.args, {}, "hero", "hero")
        self.assertEqual(result["elapsed_seconds"], 2)
        self.assertEqual(output.call_args_list, [
            call("[window case=hero elapsed=1.00s] start", flush=True),
            call("[window case=hero elapsed=3.00s] end passed", flush=True)])

    def test_failed_case_has_flushed_end_progress(self):
        with patch.object(self.smoke, "run_isolated_case", side_effect=RuntimeError("capture failed")), \
                patch("builtins.print") as output:
            with self.assertRaisesRegex(RuntimeError, "capture failed"):
                self.smoke.run_case(self.args, {}, "hero", "hero")
        self.assertIn("end failed", output.call_args.args[0])
        self.assertTrue(all(invocation.kwargs["flush"] for invocation in output.call_args_list))

    def test_predicate_wait_still_fails_after_thirty_seconds(self):
        with patch.object(self.smoke.time, "monotonic", side_effect=[0, 0, 31]), \
                patch.object(self.smoke.time, "sleep"):
            with self.assertRaisesRegex(AssertionError, "Timed out waiting for test wait"):
                self.smoke.wait_for(lambda: None, "test wait")

    def test_failed_smoke_kills_window_if_termination_times_out(self):
        process = Mock(pid=101)
        process.poll.return_value = None
        process.wait.side_effect = [subprocess.TimeoutExpired("chess3d", 10), 0]
        with patch.object(self.smoke.subprocess, "Popen", return_value=process), \
                patch.object(self.smoke.subprocess, "run", side_effect=[Mock(), RuntimeError("xdotool failed")]), \
                patch("builtins.print") as progress:
            with self.assertRaisesRegex(RuntimeError, "xdotool failed"):
                self.smoke.run_case(self.args, {"DISPLAY": ":107"}, "hero", "hero")
        process.terminate.assert_called_once_with()
        process.kill.assert_called_once_with()
        self.assertEqual(process.wait.call_args_list, [call(timeout=10), call(timeout=5)])
        self.assertIn("end failed", progress.call_args.args[0])

    def test_title_relaunch_reuses_store_after_exit_and_compares_continue_and_undo_references(self):
        outside = Mock(name="outside")

        def image(label):
            frame = Mock(name=label)
            frame.label = label
            frame.convert.return_value = frame
            frame.crop.return_value = outside
            frame.getcolors.return_value = [None] * 1001
            return frame

        expected, reference_moved = image("initial"), image("moved")
        title, initial, highlights, moved = [image(label) for label in ("title", "initial", "highlights", "moved")]
        relaunched_title = image("title")
        inexact_resume, resumed = image("inexact-resume"), image("moved")
        inexact_undo, undone = image("inexact-undo"), image("initial")
        first, second = Mock(pid=101), Mock(pid=202)
        for process in (first, second):
            process.wait.return_value = 0
            process.poll.return_value = 0
        launches = []

        def launch(command, **kwargs):
            launches.append((command, kwargs))
            if len(launches) == 1:
                Path(command[command.index("--save") + 1]).write_text(
                    json.dumps({"pieces": [None] * 32, "seed": 342}))
                return first
            first.wait.assert_called_once_with(timeout=15)
            return second

        def run(command, **kwargs):
            self.assertGreater(kwargs["timeout"], 0)
            if command[0] == str(self.args.binary):
                self.assertEqual(kwargs["timeout"], 60)
                self.assertNotIn("DISPLAY", kwargs["env"])
                return Mock()
            self.assertEqual(kwargs["timeout"], 10)
            if command[1] == "search":
                return SimpleNamespace(stdout="1" if "101" in command else "2")
            if command[1] == "getwindowgeometry":
                return SimpleNamespace(stdout="X=0\nY=0\nWIDTH=960\nHEIGHT=720\n")
            return SimpleNamespace(stdout="")

        def difference(left, right):
            if left is outside and right is outside:
                return 0
            if right in (inexact_resume, inexact_undo):
                return 0.05
            return 0 if left.label == right.label else 3

        with patch.object(self.smoke.subprocess, "Popen", side_effect=launch), \
                patch.object(self.smoke.subprocess, "run", side_effect=run) as calls, \
                patch.object(self.smoke, "close_request") as close, \
                patch.object(self.smoke.Image, "open", side_effect=[expected, reference_moved]), \
                patch.object(self.smoke.ImageGrab, "grab", side_effect=[
                    title, initial, highlights, moved, relaunched_title, inexact_resume, resumed, inexact_undo, undone]), \
                patch.object(self.smoke.ImageStat, "Stat", return_value=SimpleNamespace(stddev=[30] * 3)), \
                patch.object(self.smoke, "difference", side_effect=difference) as comparisons, \
                patch.object(self.smoke.time, "sleep"), patch("builtins.print") as progress:
            result = self.smoke.run_case(self.args, {"DISPLAY": ":107"}, "title-to-friend", "overhead",
                                         interaction="title")
        self.assertEqual(len(launches), 2)
        initial_command, relaunch_command = [entry[0] for entry in launches]
        data_dir = initial_command[initial_command.index("--data-dir") + 1]
        self.assertEqual(relaunch_command[relaunch_command.index("--data-dir") + 1], data_dir)
        for command in (initial_command, relaunch_command):
            self.assertNotIn("--moves", command)
            self.assertNotIn("--play", command)
        self.assertIs(launches[0][1]["stdout"], launches[1][1]["stdout"])
        self.assertEqual(close.call_args_list, [call("1"), call("2")])
        second.wait.assert_called_once_with(timeout=15)
        first.terminate.assert_not_called()
        second.terminate.assert_not_called()
        commands = [invocation.args[0] for invocation in calls.call_args_list]
        self.assertIn([str(self.args.xdotool), "mousemove", "--window", "2", "284", "500"], commands)
        self.assertIn(call(moved, inexact_resume), comparisons.call_args_list)
        self.assertIn(call(moved, resumed), comparisons.call_args_list)
        self.assertIn(call(initial, inexact_undo), comparisons.call_args_list)
        self.assertIn(call(initial, undone), comparisons.call_args_list)
        self.assertIn("relaunch Continue restores exact board pixels", result["controls"])
        self.assertIn("U exact board undo", result["controls"])
        messages = [invocation.args[0] for invocation in progress.call_args_list]
        for label in ("launch: headless reference capture", "launch: chess window",
                      "wait: first chess window exit", "launch: relaunch chess window with saved data",
                      "wait: relaunch title menu", "wait: Continue exact pre-relaunch pixels",
                      "wait: U exact board undo", "end passed"):
            self.assertTrue(any(message.endswith(label) for message in messages), label)
        self.assertTrue(all(invocation.kwargs["flush"] for invocation in progress.call_args_list))


if __name__ == "__main__":
    unittest.main()
