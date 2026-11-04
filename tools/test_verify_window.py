#!/usr/bin/env python3
"""Window wrapper controls with fake tools and subprocesses; no Xvfb or GPU needed."""
import os
from pathlib import Path
import runpy
import signal
import subprocess
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, patch

import verify_window


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
        popen = patch("verify_window.subprocess.Popen", return_value=self.process)
        self.popen = popen.start()
        self.addCleanup(popen.stop)
        killpg = patch("verify_window.os.killpg")
        self.killpg = killpg.start()
        self.addCleanup(killpg.stop)

    def test_command_selects_free_display_and_retains_xvfb_log(self):
        verify_window.run(self.args)
        command = self.popen.call_args.args[0]
        self.assertIn("--auto-servernum", command)
        self.assertIn("--server-num=100", command)
        self.assertIn(f"--error-file={self.args.output / 'xvfb.log'}", command)
        self.assertTrue(self.args.output.is_dir())

    def test_failure_reports_xvfb_log_tail(self):
        self.process.wait.return_value = 1

        def launch(*args, **kwargs):
            self.args.output.mkdir(parents=True, exist_ok=True)
            lines = ["old diagnostic"] + [f"Xvfb diagnostic {index}" for index in range(20)]
            (self.args.output / "xvfb.log").write_text("\n".join(lines) + "\n")
            return self.process

        self.popen.side_effect = launch
        with self.assertRaises(ValueError) as raised:
            verify_window.run(self.args)
        message = str(raised.exception)
        self.assertIn("real-window harness failed with exit 1", message)
        self.assertIn(str(self.args.output / "xvfb.log"), message)
        self.assertIn("Xvfb diagnostic 19", message)
        self.assertNotIn("old diagnostic", message)

    def test_failure_without_xvfb_log_still_reports_exit(self):
        self.process.wait.return_value = 7
        with self.assertRaisesRegex(ValueError, "real-window harness failed with exit 7"):
            verify_window.run(self.args)

    def test_timeout_reports_progress_and_xvfb_tails_with_total_bound(self):
        self.process.wait.side_effect = [subprocess.TimeoutExpired("smoke", 1200), 0]

        def launch(*args, **kwargs):
            for filename, prefix in (("progress.log", "window progress"), ("xvfb.log", "Xvfb")):
                lines = ["old diagnostic"] + [f"{prefix} {index}" for index in range(20)]
                (self.args.output / filename).write_text("\n".join(lines) + "\n")
            return self.process

        self.popen.side_effect = launch
        with self.assertRaises(ValueError) as raised:
            verify_window.run(self.args)
        message = str(raised.exception)
        self.assertIn("real-window harness timed out after 1200s", message)
        self.assertIn(str(self.args.output / "progress.log"), message)
        self.assertIn("window progress 0", message)
        self.assertIn("window progress 19", message)
        self.assertIn(str(self.args.output / "xvfb.log"), message)
        self.assertIn("Xvfb 19", message)
        self.assertNotIn("old diagnostic", message)
        self.assertEqual(self.process.wait.call_args_list[0].kwargs, {"timeout": 1200})
        self.assertEqual(self.process.wait.call_args_list[1].kwargs, {"timeout": 5})
        self.killpg.assert_called_once_with(self.process.pid, signal.SIGTERM)
        self.assertNotIn("stdout", self.popen.call_args.kwargs)
        self.assertNotIn("stderr", self.popen.call_args.kwargs)

    def test_timeout_without_logs_still_reports_total_bound(self):
        self.process.wait.side_effect = [subprocess.TimeoutExpired("smoke", 1200), 0]
        with self.assertRaisesRegex(ValueError, "real-window harness timed out after 1200s"):
            verify_window.run(self.args)

    def test_nonempty_output_is_preserved(self):
        self.args.output.mkdir()
        sentinel = self.args.output / "xvfb.log"
        sentinel.write_text("previous evidence")
        with self.assertRaisesRegex(ValueError, "--output must be empty"):
            verify_window.run(self.args)
        self.assertEqual(sentinel.read_text(), "previous evidence")
        self.popen.assert_not_called()

    def test_child_uses_display_selected_by_xvfb_run(self):
        with patch.dict(os.environ, {"DISPLAY": ":107"}), patch.dict("sys.modules", {"PIL": Mock()}):
            harness = runpy.run_path(str(verify_window.ROOT / "tests/window_smoke.py"))
        self.assertEqual(harness["DISPLAY"], ":107")


class WindowWaitTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.output = Path(self.temp.name)
        self.log = self.output / "overhead.log"
        self.log.write_text("old diagnostic\n" + "".join(f"chess log {index}\n" for index in range(20)))
        with patch.dict(os.environ, {"DISPLAY": ":107"}), patch.dict("sys.modules", {"PIL": Mock()}):
            harness = runpy.run_path(str(verify_window.ROOT / "tests/window_smoke.py"))
        self.wait_for = harness["wait_for"]
        self.now = 0.0
        clock = SimpleNamespace(monotonic=lambda: self.now, sleep=self.advance)
        self.print = Mock()
        globals_patch = patch.dict(self.wait_for.__globals__, {"time": clock, "print": self.print})
        globals_patch.start()
        self.addCleanup(globals_patch.stop)
        self.process = Mock()
        self.process.poll.return_value = None

    def advance(self, seconds):
        self.now += seconds

    def wait(self, predicate):
        return self.wait_for(predicate, "chess window", process=self.process,
                             case="overhead", output=self.output, started=0.0)

    def test_exited_process_fails_immediately_with_exit_code_and_log_tail(self):
        self.process.poll.return_value = 17
        predicate = Mock()
        with self.assertRaises(AssertionError) as raised:
            self.wait(predicate)
        message = str(raised.exception)
        self.assertIn("Chess process exited with code 17 waiting for chess window (case overhead)", message)
        self.assertIn(str(self.log), message)
        self.assertIn("chess log 19", message)
        self.assertNotIn("old diagnostic", message)
        predicate.assert_not_called()
        self.assertEqual(self.now, 0.0)

    def test_exit_during_predicate_is_reported_before_success(self):
        self.process.poll.side_effect = [None, 9]
        with self.assertRaisesRegex(AssertionError, "Chess process exited with code 9"):
            self.wait(lambda: "window id")
        self.assertEqual(self.now, 0.0)

    def test_exit_during_failed_predicate_reports_process_exit(self):
        self.process.poll.side_effect = [None, 9]
        predicate = Mock(side_effect=subprocess.CalledProcessError(1, "xdotool"))
        with self.assertRaisesRegex(AssertionError, "Chess process exited with code 9"):
            self.wait(predicate)
        self.assertEqual(self.now, 0.0)

    def test_predicate_failure_while_alive_is_not_retried(self):
        predicate = Mock(side_effect=subprocess.CalledProcessError(1, "xdotool"))
        with self.assertRaises(subprocess.CalledProcessError):
            self.wait(predicate)
        predicate.assert_called_once_with()

    def test_slow_live_process_can_succeed_beyond_thirty_seconds(self):
        self.assertEqual(self.wait(lambda: "window id" if self.now >= 45 else None), "window id")
        self.assertGreaterEqual(self.now, 45)
        lines = (self.output / "progress.log").read_text().splitlines()
        self.assertEqual(len(lines), 2)
        self.assertIn("case=overhead step=chess window state=waiting elapsed=0.00s", lines[0])
        self.assertIn(f"case=overhead step=chess window state=ready elapsed={self.now:.2f}s", lines[1])
        for call, line in zip(self.print.call_args_list, lines):
            self.assertEqual(call.args, (line,))
            self.assertEqual(call.kwargs, {"flush": True})

    def test_live_process_without_progress_fails_at_stall_bound(self):
        with self.assertRaisesRegex(AssertionError, "after 180s without progress"):
            self.wait(lambda: None)
        self.assertGreaterEqual(self.now, 180)
        self.assertLess(self.now, 180.1)
        self.assertEqual(len((self.output / "progress.log").read_text().splitlines()), 1)


if __name__ == "__main__":
    unittest.main()
