"""Bootstrap safety controls; use disposable local paths, never the sibling engine."""
import subprocess
import tempfile
import unittest
from pathlib import Path

from bootstrap_engine import bootstrap, validate_revision
from verify_chess3d import repository_info


class BootstrapTests(unittest.TestCase):
    def test_revision_is_explicit_and_cannot_be_a_git_option(self):
        for value in ("", "--upload-pack=evil", "../main", "main:other", "main\nother", "a b"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                validate_revision(value)
        for value in ("main", "v0.1.0", "refs/heads/main", "a" * 40):
            self.assertEqual(validate_revision(value), value)

    def test_failed_fetch_does_not_publish_partial_engine(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            destination = root / "real-disunity"
            with self.assertRaises((ValueError, subprocess.CalledProcessError)):
                bootstrap(destination, "main", repository=str(root / "missing-remote"))
            self.assertFalse(destination.exists())
            self.assertEqual(list(root.iterdir()), [])

    def test_existing_noncheckout_is_not_overwritten(self):
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / "real-disunity"
            destination.mkdir()
            sentinel = destination / "Cargo.toml"
            sentinel.write_text("work in progress")
            with self.assertRaisesRegex(ValueError, "existing"):
                bootstrap(destination, "main")
            self.assertEqual(sentinel.read_text(), "work in progress")
            self.assertEqual(list(destination.iterdir()), [sentinel])

    def test_dirty_existing_checkout_is_not_changed_or_fetched(self):
        with tempfile.TemporaryDirectory() as temp:
            destination = Path(temp) / "real-disunity"
            subprocess.run(["git", "init", "-q", str(destination)], check=True)
            sentinel = destination / "uncommitted"
            sentinel.write_text("in progress")
            before = sorted(str(p.relative_to(destination)) for p in destination.rglob("*"))
            with self.assertRaisesRegex(ValueError, "uncommitted"):
                bootstrap(destination, "main")
            self.assertEqual(sentinel.read_text(), "in progress")
            self.assertEqual(before, sorted(str(p.relative_to(destination)) for p in destination.rglob("*")))

    def test_provenance_handles_unborn_head_without_fabricating_revision(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            subprocess.run(["git", "init", "-q", str(root)], check=True)
            (root / "source.rs").write_text("fn main() {}")
            receipt = repository_info(root)
            self.assertIsNone(receipt["revision"])
            self.assertTrue(receipt["dirty"])
            self.assertIn("?? source.rs", receipt["status"])


if __name__ == "__main__":
    unittest.main()
