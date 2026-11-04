#!/usr/bin/env python3
"""Window wrapper controls with fake tools and subprocesses; no Xvfb or GPU needed."""
import os
from pathlib import Path
import runpy
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
        killpg.start()
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


if __name__ == "__main__":
    unittest.main()
