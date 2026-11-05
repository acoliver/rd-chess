"""CPU-only tests of the streaming puzzle subset selector."""
import csv
import io
import json
from pathlib import Path
import shutil
import subprocess
import unittest

from select_puzzles import SOURCE, cli_zstd, select, rating_band


class SelectionTests(unittest.TestCase):
    def sample(self):
        stream = io.StringIO()
        writer = csv.writer(stream)
        writer.writerow(["PuzzleId", "FEN", "Moves", "Rating", "Themes"])
        for theme in ("fork", "pin"):
            for rating in (900, 1500, 2100):
                writer.writerow([f"{theme}{rating}", "test fen", "a2a3 a7a6", rating, theme])
        stream.seek(0)
        return csv.DictReader(stream)

    def test_balanced_inline_csv(self):
        rows = select(self.sample(), themes=("fork", "pin"), quotas=(1, 1, 1))
        self.assertEqual(len(rows), 6)
        self.assertEqual({rating_band(p["Rating"]) for p in rows}, {0, 1, 2})
        self.assertTrue(all(set(p) == {"PuzzleId", "FEN", "Moves", "Rating", "Themes"} for p in rows))
        self.assertTrue(SOURCE.endswith(".csv.zst"))

    def test_duplicate_ids_and_multi_themes_do_not_fill_twice(self):
        rows = [{"PuzzleId": "same", "FEN": "fen", "Moves": "a2a3 a7a6", "Rating": "900", "Themes": "fork pin"}] * 3
        with self.assertRaisesRegex(ValueError, "unfilled"):
            select(rows, themes=("fork", "pin"), quotas=(1, 0, 0))

    def test_stops_consuming_when_quotas_are_full(self):
        def rows():
            yield {"PuzzleId": "a", "FEN": "fen", "Moves": "a2a3 a7a6", "Rating": "900", "Themes": "fork"}
            raise AssertionError("stream consumed past completed quota")
        self.assertEqual(len(select(rows(), themes=("fork",), quotas=(1, 0, 0))), 1)

    def test_rating_boundaries(self):
        self.assertEqual([rating_band(n) for n in (0, 1199, 1200, 1799, 1800, 4000)], [0, 0, 1, 1, 2, 2])

    def test_committed_pack_reproduces_quotas(self):
        path = Path(__file__).resolve().parents[1] / "assets/puzzles/first-pack.json"
        pack = json.loads(path.read_text())
        self.assertEqual(pack["source"], SOURCE)
        self.assertEqual(select(pack["puzzles"]), pack["puzzles"])
        self.assertEqual(len(pack["puzzles"]), 300)
        self.assertLess(path.stat().st_size, 200000)

    def test_cli_zstd_decompresses_a_stream(self):
        executable = shutil.which("zstd")
        if executable is None:
            self.skipTest("optional zstd fallback executable is not installed")
        sample = "PuzzleId,FEN,Moves,Rating,Themes\na,fen,a2a3 a7a6,900,fork\n"
        compressed = subprocess.run([executable, "-q", "-c"], input=sample.encode(),
                                    check=True, capture_output=True).stdout
        with cli_zstd(io.BytesIO(compressed), executable) as text:
            puzzles = select(csv.DictReader(text), themes=("fork",), quotas=(1, 0, 0))
        self.assertEqual(puzzles[0]["PuzzleId"], "a")


if __name__ == "__main__":
    unittest.main()
