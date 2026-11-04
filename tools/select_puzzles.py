#!/usr/bin/env python3
"""Stream a small CC0 Lichess subset, with fixed theme/rating quotas."""
import argparse
import contextlib
import csv
import io
import json
import shutil
import subprocess
import sys
import threading
import urllib.request

SOURCE = "https://database.lichess.org/lichess_db_puzzle.csv.zst"
THEMES = ("fork", "pin", "skewer", "mateIn1", "mateIn2", "hangingPiece")
FIELDS = ("PuzzleId", "FEN", "Moves", "Rating", "Themes")


def rating_band(rating):
    """Bands are exercise difficulty, not estimates of player game strength."""
    return 0 if int(rating) < 1200 else 1 if int(rating) < 1800 else 2


def select(rows, themes=THEMES, quotas=(17, 17, 16)):
    """First-fit in source order; assign each unique ID to one quota only."""
    remaining = {(theme, band): quota for theme in themes
                 for band, quota in enumerate(quotas)}
    selected, seen = [], set()
    for row in rows:
        if row["PuzzleId"] in seen:
            continue
        band = rating_band(row["Rating"])
        tags = row["Themes"].split()
        theme = next((t for t in themes if t in tags and remaining[t, band]), None)
        if theme is None:
            continue
        puzzle = {field: row[field] for field in FIELDS}
        puzzle["Rating"] = int(puzzle["Rating"])
        selected.append(puzzle)
        seen.add(puzzle["PuzzleId"])
        remaining[theme, band] -= 1
        if not any(remaining.values()):
            return selected
    missing = {f"{theme}/{band}": n for (theme, band), n in remaining.items() if n}
    raise ValueError(f"unfilled quotas: {missing}")


@contextlib.contextmanager
def cli_zstd(response, executable):
    """Feed decoded HTTPS bytes in bounded chunks, not the encrypted socket fd."""
    errors, stopping = [], threading.Event()
    with subprocess.Popen([executable, "-dc"], stdin=subprocess.PIPE,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE) as process:
        def feed():
            try:
                shutil.copyfileobj(response, process.stdin, length=65536)
            except (OSError, ValueError) as error:
                if not stopping.is_set():
                    errors.append(error)
            finally:
                try:
                    process.stdin.close()
                except BrokenPipeError as error:
                    if not stopping.is_set():
                        errors.append(error)
        worker = threading.Thread(target=feed)
        worker.start()
        try:
            with io.TextIOWrapper(process.stdout, encoding="utf-8", newline="") as text:
                yield text
        finally:
            stopping.set()
            if process.poll() is None:
                process.terminate()
            process.wait()
            response.close()
            worker.join()
        if errors:
            raise errors[0]
        if process.returncode not in (0, -15):
            raise RuntimeError(f"zstd failed: {process.stderr.read().decode('utf-8', errors='replace')}")


@contextlib.contextmanager
def remote_csv():
    """No full-export download or temporary file is required."""
    try:
        from compression import zstd
    except ImportError:
        zstd = None
    with urllib.request.urlopen(SOURCE, timeout=60) as response:
        if zstd is not None:
            with zstd.ZstdFile(response) as decoded:
                with io.TextIOWrapper(decoded, encoding="utf-8", newline="") as text:
                    yield text
        else:
            executable = shutil.which("zstd")
            if executable is None:
                raise RuntimeError("Need compression.zstd or zstd on PATH; see assets/puzzles/README.md")
            with cli_zstd(response, executable) as text:
                yield text


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--csv", help="Read an already decompressed CSV, or - for stdin")
    parser.add_argument("--output", default="assets/puzzles/first-pack.json")
    args = parser.parse_args()
    with contextlib.ExitStack() as stack:
        if args.csv == "-":
            stream = sys.stdin
        elif args.csv:
            stream = stack.enter_context(open(args.csv, encoding="utf-8", newline=""))
        else:
            stream = stack.enter_context(remote_csv())
        puzzles = select(csv.DictReader(stream))
    with open(args.output, "w", encoding="utf-8") as output:
        json.dump({"version": 1, "source": SOURCE, "puzzles": puzzles}, output,
                  separators=(",", ":"))
        output.write("\n")
    print(f"Selected {len(puzzles)} puzzles into {args.output}")


if __name__ == "__main__":
    main()
