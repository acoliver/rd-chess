# Bundled Lichess puzzle subset

`first-pack.json` contains 300 puzzles selected from the Lichess puzzle database
on 2026-11-04. The source export reported an update date of 2026-11-03.
The installed game embeds the subset and requires no network access.

Source: <https://database.lichess.org/lichess_db_puzzle.csv.zst>
Database and licence statement: <https://database.lichess.org/#puzzles>

## Data notice and licence

Lichess releases its database exports under **Creative Commons CC0 1.0 Universal**.
The database page was checked on 2026-11-04. It states:

> Database exports are released under the Creative Commons CC0 license.
> Use them for research, commercial purpose, publication, anything you like.
> You can download, modify and redistribute them, without asking for permission.

CC0 legal text: <https://creativecommons.org/publicdomain/zero/1.0/legalcode>

This notice credits Lichess as the source. Apart from the licence quotation
above, this pack contains only exported puzzle data. No site icons, source code
or book material is bundled. The game's source and original course retain their
existing repository licence. The CC0 notice does not change that licence.

## Subset and reproduction

The committed file is 54,576 bytes. Ratings span 473 through 2672. Selection
assigns 50 distinct puzzles to each of `fork`, `pin`, `skewer`, `mateIn1`,
`mateIn2` and `hangingPiece`. Each theme has quotas of 17 below 1200, 17 from
1200 through 1799, and 16 at 1800 or above. This gives band totals of 102, 102
and 96. A row can have several themes but fills only the first available quota
in that theme order. The file retains all original theme tags, so theme
occurrence totals exceed the assignment quotas. There are 126 mate-tagged rows.
These are source puzzle ratings, not measurements of the learner's game strength.

From the repository root:

```sh
python3 tools/select_puzzles.py
# Alternatively, use a local decompressed export:
python3 tools/select_puzzles.py --csv /path/to/lichess_db_puzzle.csv
# Or stream decompression with an installed zstd executable:
zstd -dc /path/to/lichess_db_puzzle.csv.zst | python3 tools/select_puzzles.py --csv -
```

The script uses Python's standard library, preferring `compression.zstd` where
available. Otherwise it uses `zstd -dc` from PATH, feeding the HTTPS response in
bounded chunks. If neither decompressor is available, install zstd or provide
an already decompressed CSV. No Python package is required. The script stops
reading as soon as the quotas fill and does not store the full database. It
fails if a supplied CSV cannot fill the quotas. Selection is deterministic for
the same source row order; the upstream rolling export may change over time.
The Rust puzzle suite validates every selected move before changes are committed.

The input CSV fields documented by Lichess are `PuzzleId,FEN,Moves,Rating,
RatingDeviation,Popularity,NbPlays,Themes,GameUrl,OpeningTags,DailyDate`.
The compact version-1 JSON retains `PuzzleId`, `FEN`, `Moves`, `Rating` and
`Themes`, plus the export URL once in the pack header.

**The FEN precedes the opponent's setup move.** The first UCI token in `Moves`
is applied before presenting the position. The solver starts with the second
token, then alternates with scripted replies. Mate-tagged solutions must finish
in checkmate. Any legal mating move is accepted for mate goals, including
alternative final mates in mate-in-one and mate-in-two puzzles. Other moves
must match the source line; the trainer makes no independent search claim.

```sh
cargo run --locked -p chess3d -- --puzzle 00008
cargo test --locked -p chess3d --test puzzles
python3 -m unittest discover -s tools -p 'test_select_puzzles.py'
```
