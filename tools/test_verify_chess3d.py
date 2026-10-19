#!/usr/bin/env python3
"""Behavioral controls for the acceptance decoder and image gates; no GPU substitutes."""
import hashlib
import os
import struct
import sys
import tempfile
import unittest
import zlib
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch

from chess3d_image import compare, decode_png, image_stats, validate_image
from verify_chess3d import Acceptance


def chunk(kind, data):
    return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))


def png_bytes(width, height, rows, color=6):
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, color, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


class ImageTests(unittest.TestCase):
    def decode(self, data):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "image.png"
            path.write_bytes(data)
            return decode_png(path)

    def test_all_png_filters_decode_to_identical_rgba(self):
        # Constant first row, then a different constant row: exercises left, above,
        # upper-left and both branches of Paeth without using the decoder to encode.
        expected = bytes([20, 30, 40, 255] * 2 + [25, 35, 45, 255] * 2)
        encoded_rows = [
            bytes([0, 20, 30, 40, 255, 20, 30, 40, 255, 0, 25, 35, 45, 255, 25, 35, 45, 255]),
            bytes([1, 20, 30, 40, 255, 0, 0, 0, 0, 1, 25, 35, 45, 255, 0, 0, 0, 0]),
            bytes([2, 20, 30, 40, 255, 20, 30, 40, 255, 2, 5, 5, 5, 0, 5, 5, 5, 0]),
            bytes([3, 20, 30, 40, 255, 10, 15, 20, 128, 3, 15, 20, 25, 128, 3, 3, 3, 0]),
            bytes([4, 20, 30, 40, 255, 0, 0, 0, 0, 4, 5, 5, 5, 0, 0, 0, 0, 0]),
        ]
        for rows in encoded_rows:
            with self.subTest(filter=rows[0]):
                self.assertEqual(self.decode(png_bytes(2, 2, rows)), (2, 2, expected))

    def test_paeth_can_select_the_upper_left_predictor(self):
        rows = bytes([0, 100, 100, 100, 255, 20, 20, 20, 255,
                      4, 100, 100, 100, 0, 236, 236, 236, 0])
        expected = bytes([100, 100, 100, 255, 20, 20, 20, 255,
                          200, 200, 200, 255, 80, 80, 80, 255])
        self.assertEqual(self.decode(png_bytes(2, 2, rows)), (2, 2, expected))

    def test_rgb_expands_to_opaque_rgba(self):
        self.assertEqual(self.decode(png_bytes(1, 1, b"\0\1\2\3", color=2)),
                         (1, 1, bytes([1, 2, 3, 255])))

    def test_corruption_and_truncation_fail(self):
        data = png_bytes(1, 1, b"\0\1\2\3\xff")
        for bad in [data[:-5], data[:45] + bytes([data[45] ^ 1]) + data[46:],
                    png_bytes(1, 1, b"\5\1\2\3\xff"),
                    png_bytes(2, 1, b"\0\1\2\3\xff")]:
            with self.assertRaises(ValueError):
                self.decode(bad)

    def test_oversized_file_and_dimensions_are_rejected(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp) / "oversize.png"
            with path.open("wb") as stream:
                stream.truncate(80 * 1024 * 1024 + 1)
            with self.assertRaisesRegex(ValueError, "file budget"):
                decode_png(path)
        with self.assertRaisesRegex(ValueError, "dimensions"):
            self.decode(png_bytes(4097, 1, b""))

    def test_unsupported_modes_chunks_and_zlib_trailing_data_fail(self):
        header = chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        prefix = bytes.fromhex("89504e470d0a1a0a") + header
        tail = chunk(b"IEND", b"")
        for bad in [png_bytes(1, 1, b"\0\1", color=3),
                    prefix + chunk(b"tRNS", b"abcdef") + tail,
                    prefix + chunk(b"IDAT", zlib.compress(b"\0\1\2\3\xff") + b"junk") + tail,
                    png_bytes(1, 1, b"\0" + bytes(1024 * 1024))]:
            with self.subTest(length=len(bad)), self.assertRaises(ValueError):
                self.decode(bad)

    def test_consecutive_idat_and_ancillary_chunks_preserve_pixels(self):
        header = chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        compressed = zlib.compress(b"\0\1\2\3\xff")
        data = (b"\x89PNG\r\n\x1a\n" + header + chunk(b"tEXt", b"key\0before")
                + chunk(b"IDAT", compressed[:3]) + chunk(b"IDAT", b"")
                + chunk(b"IDAT", compressed[3:]) + chunk(b"tEXt", b"key\0after")
                + chunk(b"IEND", b""))
        self.assertEqual(self.decode(data), (1, 1, bytes([1, 2, 3, 255])))

    def test_crc_valid_invalid_chunk_names_are_rejected(self):
        header = chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        image = chunk(b"IDAT", zlib.compress(b"\0\1\2\3\xff"))
        for kind in (b"t1Xt", b"t\xffXt", b"texT", b"tExt", b"tE\0t"):
            data = (b"\x89PNG\r\n\x1a\n" + header + chunk(kind, b"")
                    + image + chunk(b"IEND", b""))
            with self.subTest(kind=kind), self.assertRaisesRegex(ValueError, "chunk name"):
                self.decode(data)

    def test_crc_valid_invalid_chunk_order_is_rejected(self):
        header = chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        compressed = zlib.compress(b"\0\1\2\3\xff")
        image = chunk(b"IDAT", compressed)
        text = chunk(b"tEXt", b"key\0value")
        end = chunk(b"IEND", b"")
        cases = {
            "ancillary before header": text + header + image + end,
            "data before header": image + header + end,
            "duplicate header": header + header + image + end,
            "missing data": header + end,
            "nonconsecutive data": (header + chunk(b"IDAT", compressed[:3]) + text
                                    + chunk(b"IDAT", compressed[3:]) + end),
            "empty nonconsecutive data": header + image + text + chunk(b"IDAT", b"") + end,
        }
        for name, chunks in cases.items():
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "header|IDAT"):
                self.decode(b"\x89PNG\r\n\x1a\n" + chunks)

    def test_optional_palette_is_explicitly_unsupported(self):
        header = chunk(b"IHDR", struct.pack(">IIBBBBB", 1, 1, 8, 6, 0, 0, 0))
        image = chunk(b"IDAT", zlib.compress(b"\0\1\2\3\xff"))
        palette = chunk(b"PLTE", b"\1\2\3")
        for chunks in (palette + image, image + palette, palette + palette + image):
            data = b"\x89PNG\r\n\x1a\n" + header + chunks + chunk(b"IEND", b"")
            with self.subTest(chunks=chunks), self.assertRaisesRegex(ValueError, "unsupported PNG chunk"):
                self.decode(data)

    def test_pixel_comparison_ignores_container_encoding(self):
        image = self.decode(png_bytes(1, 1, b"\0\1\2\3\xff"))
        self.assertEqual(compare(image, image)["changed_pixels"], 0)
        other = (1, 1, bytes([2, 2, 3, 255]))
        self.assertEqual(compare(image, other)["changed_pixels"], 1)
        with self.assertRaises(ValueError):
            compare(image, (2, 1, other[2] * 2))

    def test_blank_transparent_and_wrong_dimensions_fail(self):
        for pixels in [bytes([0, 0, 0, 255] * 64), bytes([255, 255, 255, 0] * 64)]:
            stats = image_stats((8, 8, pixels))
            with self.assertRaises(ValueError):
                validate_image(stats, (8, 8))
        with self.assertRaises(ValueError):
            validate_image(image_stats((8, 8, bytes([0, 0, 0, 255] * 64))), (9, 8))


class HarnessTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.icd = self.root / "test-icd.json"
        self.icd.write_text("{}")
        self.args = SimpleNamespace(output=self.root / "evidence", target_dir=self.root / "target",
                                    timeout=2, baseline_dir=None)
        self.environment = patch.dict(os.environ, {"VK_DRIVER_FILES": str(self.icd)})
        self.environment.start()
        self.addCleanup(self.environment.stop)

    def test_acceptance_requires_gpu_mode_and_standalone_assets(self):
        harness = Acceptance(self.args)
        self.assertEqual(harness.env.get("CHESS3D_REQUIRE_GPU"), "1")
        self.assertTrue((harness.fixture_dir / "initial.json").is_file())
        self.assertTrue((harness.fixture_dir / "study.json").is_file())
        self.assertEqual(harness.fixture_dir.parent.name, "assets")

    def test_icd_file_budget_is_enforced(self):
        with self.icd.open("wb") as stream:
            stream.truncate(1024 * 1024 + 1)
        with self.assertRaisesRegex(ValueError, "ICD file budget"):
            Acceptance(self.args)

    def test_icd_at_file_budget_is_hashed(self):
        data = b" " * (1024 * 1024)
        self.icd.write_bytes(data)
        harness = Acceptance(self.args)
        self.assertEqual(harness.report["icd_sha256"], hashlib.sha256(data).hexdigest())

    def test_icd_growth_after_size_check_is_rejected(self):
        fstat = os.fstat

        def grow_after_stat(fd):
            metadata = fstat(fd)
            with self.icd.open("ab") as stream:
                stream.write(b" " * (1024 * 1024))
            return metadata

        with patch("verify_chess3d.os.fstat", side_effect=grow_after_stat):
            with self.assertRaisesRegex(ValueError, "ICD file budget"):
                Acceptance(self.args)

    def test_icd_must_be_a_regular_file(self):
        fifo = self.root / "icd-fifo"
        os.mkfifo(fifo)
        for path in (self.root, fifo):
            with patch.dict(os.environ, {"VK_DRIVER_FILES": str(path)}):
                with self.subTest(path=path), self.assertRaisesRegex(ValueError, "regular file"):
                    Acceptance(self.args)

    def test_relative_icd_is_resolved_from_invocation_directory_for_children(self):
        previous = Path.cwd()
        try:
            os.chdir(self.root)
            with patch.dict(os.environ, {"VK_DRIVER_FILES": self.icd.name}):
                harness = Acceptance(self.args)
        finally:
            os.chdir(previous)
        self.assertEqual(harness.env["VK_DRIVER_FILES"], str(self.icd))
        self.assertEqual(harness.report["environment"]["VK_DRIVER_FILES"], str(self.icd))
        text = harness.command("relative-icd", [sys.executable, "-c",
                               "import os; from pathlib import Path; "
                               "p = Path(os.environ['VK_DRIVER_FILES']); "
                               "assert p.is_absolute(); print(p.read_text())"])
        self.assertEqual(text.strip(), "{}")
        self.assertEqual(harness.report["commands"][0]["vk_driver_files"], str(self.icd))
        self.assertEqual(harness.report["icd_sha256"], hashlib.sha256(b"{}").hexdigest())

    def test_nonempty_output_is_preserved(self):
        self.args.output.mkdir()
        sentinel = self.args.output / "report.json"
        sentinel.write_text("previous evidence")
        with self.assertRaisesRegex(ValueError, "empty"):
            Acceptance(self.args)
        self.assertEqual(sentinel.read_text(), "previous evidence")

    def test_target_and_output_cannot_overlap(self):
        for target in (self.args.output, self.args.output / "target", self.root):
            self.args.target_dir = target
            with self.subTest(target=target), self.assertRaisesRegex(ValueError, "overlap"):
                Acceptance(self.args)

    def test_timeout_must_be_positive_and_bounded(self):
        for timeout in (0, -1, 901):
            self.args.timeout = timeout
            with self.subTest(timeout=timeout), self.assertRaisesRegex(ValueError, "timeout"):
                Acceptance(self.args)

    def test_real_subprocess_failure_retains_log_and_diagnostic(self):
        harness = Acceptance(self.args)
        text = harness.command("failure", [sys.executable, "-c",
                               "import sys; print('expected error'); sys.exit(7)"],
                               success=False, diagnostic="expected error")
        self.assertIn("expected error", text)
        self.assertEqual(harness.report["commands"][0]["returncode"], 7)
        self.assertIn("expected error", (self.args.output / "failure.log").read_text())
        with self.assertRaisesRegex(ValueError, "unexpected exit"):
            harness.command("unexpected", [sys.executable, "-c", "raise SystemExit(4)"])

    def test_real_subprocess_deadline_is_reported_and_reaped(self):
        self.args.timeout = 0.2
        harness = Acceptance(self.args)
        with self.assertRaisesRegex(ValueError, "deadline"):
            harness.command("deadline", [sys.executable, "-c",
                                         "import threading; threading.Event().wait(30)"])
        receipt = harness.report["commands"][0]
        self.assertIsNotNone(receipt["returncode"])
        self.assertIn("deadline", receipt["error"])

    def test_subprocess_output_budget_is_enforced(self):
        harness = Acceptance(self.args)
        with patch("verify_chess3d.MAX_COMMAND_OUTPUT", 1024):
            with self.assertRaisesRegex(ValueError, "output budget"):
                harness.command("noisy", [sys.executable, "-c",
                                         "import os; os.write(1, b'x' * 65536)"])
        self.assertLessEqual((self.args.output / "noisy.log").stat().st_size, 2048)

    def test_command_log_cannot_escape_output_directory(self):
        harness = Acceptance(self.args)
        with self.assertRaisesRegex(ValueError, "command name"):
            harness.command("../escaped", [sys.executable, "-c", "pass"])
        self.assertFalse((self.root / "escaped.log").exists())


if __name__ == "__main__":
    unittest.main()
