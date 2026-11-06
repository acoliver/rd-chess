"""Keyboard save/rename smoke controls without an X display or child processes."""
import tempfile
from pathlib import Path
from types import SimpleNamespace
import unittest
from unittest.mock import Mock, call

from test_verify_window import load_smoke


class RenameControlsTests(unittest.TestCase):
    def setUp(self):
        self.smoke = load_smoke()
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        args = SimpleNamespace(binary=self.root / "chess3d", xdotool=self.root / "xdotool",
                               output=self.root)
        progress = Mock()
        progress.name = "title-to-friend"
        self.case = self.smoke.SmokeCase(args, {},
                                       {"view": "overhead", "load": None, "interaction": "title"},
                                       str(self.root), progress)
        self.case.window = "1"
        self.case.records = []
        self.current = self.root / "slot-current.rds"
        self.current.write_bytes(b"unchanged current game")
        self.slot = self.root / "slot-slot-1.rds"
        self.moved = object()
        self.case.matching = Mock(return_value=self.moved)
        self.changed_current = False
        self.invocations = 0
        self.case.xdo = Mock(side_effect=self.execute)
        self.case.wait = Mock(side_effect=self.wait)

    def execute(self, *args):
        if self.invocations == 0:
            self.slot.write_bytes(b"Two players, move 1")
        if self.invocations == 3:
            self.slot.write_bytes(b"Smoke opening")
            if self.changed_current:
                self.current.write_bytes(b"changed")
        self.invocations += 1

    def wait(self, predicate, label):
        value = predicate()
        self.assertTrue(value, label)
        return value

    def test_rename_uses_proven_keyboard_sequence_and_waits_for_saved_bytes(self):
        self.case.rename_slot_by_keys(self.moved)
        self.assertEqual(self.case.xdo.call_args_list, [
            call("key", "--window", "1", "Escape", "Home", "Tab", "Tab", "Tab", "Tab", "Tab",
                 "Return", "Return", "Return"),
            call("key", "--window", "1", "Tab", "Return", "Home", "shift+End"),
            call("type", "--window", "1", "--clearmodifiers", "Smoke opening"),
            call("key", "--window", "1", "Return"),
            call("key", "--window", "1", "Escape", "Escape"),
        ])
        self.assertEqual([entry.args[1] for entry in self.case.wait.call_args_list], [
            "default label saved in slot 1", "renamed slot persisted",
            "rename returns to exact played board",
        ])
        self.assertEqual(self.current.read_bytes(), b"unchanged current game")
        self.assertEqual(self.slot.read_bytes(), b"Smoke opening")
        self.case.matching.assert_called_once_with(self.moved, 0)
        self.assertEqual(self.case.records, ["keyboard save and rename preserves the current game"])

    def test_rename_fails_if_the_current_game_changes(self):
        self.changed_current = True
        with self.assertRaisesRegex(AssertionError, "rename changed the current game"):
            self.case.rename_slot_by_keys(self.moved)
        self.assertEqual(self.case.xdo.call_count, 4)
        self.case.matching.assert_not_called()
        self.assertEqual(self.case.records, [])


if __name__ == "__main__":
    unittest.main()
