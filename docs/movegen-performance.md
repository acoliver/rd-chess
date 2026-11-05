# Move generation is faster without changing the search tree

Refs 1925d5ae4e3d. CPU-only release measurements on the M4 Max, starting from
hub main `861d178`. No search budget, depth, evaluation weight, ordering priority
or policy rule changed. Existing profiles therefore keep their playing behaviour.

## Baseline and method

The baseline commit `6b36ccf` contains only benchmarks and the regression fixture,
with no production changes. The fixture was recorded and then tested against the
original implementation before optimization. It covers ten positions at all eight
actual ladder budgets, with a fresh 4,096-slot table per analysis. Each row stores
node count, completed depth, score, full PV, every ordered candidate and the
seed-42 policy reply. It includes the opening after e2-e4, where Pip plays g8-f6.

Ignored tests use `std::time::Instant`, without `cargo bench` or new dependencies:

* Perft counts legal leaf nodes, using copy-make between internal nodes and a
  move-count shortcut at depth one, identically before and after.
* Fixed search visits 20,000 nodes at each of the ten FENs in
  `src/search/regression.rs`, with a fresh 65,536-slot table and no depth cap.
  Each run therefore visits exactly 200,000 nodes. Reported time includes FEN
  import and table construction. Search itself is single-threaded.
* Component measurements perform 100,000 calls per operation with `black_box`.
  These isolated costs identify expensive operations; they are not sampled
  percentages of total search time.

All cargo commands unset `RUSTUP_TOOLCHAIN`, `CARGO` and `RUST_RECURSION_COUNT`,
set `CARGO_INCREMENTAL=0`, use `nice -n 10`, and apply the specified SSH rewrite.
Benchmarks run sequentially, with no concurrent verification jobs from this task.
Other sessions on this shared workstation were not stopped.

## Where the baseline spent work

Baseline initial/Kiwipete component costs, in ns/call, were:

| Operation | Initial | Kiwipete |
| --- | ---: | ---: |
| Legal generation | 766.5 | 1,652.4 |
| Public play plus undo | 879.3 | 2,070.5 |
| Terminal status | 879.3 | 1,884.2 |
| Full position hash | 94.9 | 85.6 |
| Evaluation | 2,546.7 | 2,747.1 |
| Core copy-make alone | 11.7 | 11.5 |

Generation copied the 128-entry mailbox and scanned attacks for every pseudo-legal
candidate. Search then regenerated a complete legal list to validate each move
in `play`, generated another list in `status`, and often generated another during
expansion. Repeating the legality work cost much more than an individual copy.
Committed moves also copied the entire board into undo history and rescanned all
pieces for their hash.

## Changes

Compile-time knight, king and pawn target tables are derived from this project's
movement offsets. Their target order is checked against the original mailbox
steps. Slider generation retains the existing rays and their order. No external
engine code, magic numbers or attack tables were used.

A king-ray scan computes pinned origins and a check-evasion mask once per legal
list. Ordinary moves must reach the evasion mask and keep any pinned piece on
its king ray. Intersecting checker masks excludes non-king double-check evasions.
King moves and en passant still use full copy-make safety tests because they
change the attack target or remove an additional blocker. Castling retains its
transit and destination checks. Filtering remains stable, preserving every move's
original relative position.

An internal `play_generated` path applies only moves freshly generated from the
same state. It has a debug legality assertion. Public `play` still revalidates
caller-supplied and stale moves, and both paths check counter overflow before
mutation. Search uses the internal path, including its existing policy scans.

Committed make/unmake now saves two changed squares for normal moves, three for
en passant and four for castling, plus the previous metadata. It mutates the board
in place and restores those squares on undo. Incremental hashing XORs out old and
in new piece-square, castling, turn and legally usable en-passant keys. The original
key generator and key domains are unchanged. Full hashing remains available for
FEN import and independent regression comparisons.

## Three-run results

The first completed measurements after all production changes gave the following
nodes/s. Medians are taken across the three runs, not across individual FENs.

| Workload | Before runs 1 / 2 / 3 | Before median | After runs 1 / 2 / 3 | After median | Ratio |
| --- | --- | ---: | --- | ---: | ---: |
| Initial perft depth 5 | 22,811,374 / 23,939,949 / 22,663,901 | 22,811,374 | 71,100,288 / 70,743,082 / 71,859,401 | 71,100,288 | 3.12x |
| Kiwipete perft depth 4 | 25,033,610 / 25,310,846 / 25,398,831 | 25,310,846 | 81,453,309 / 81,221,684 / 80,572,100 | 81,221,684 | 3.21x |
| Fixed ten-FEN search | 228,831 / 231,789 / 224,905 | 228,831 | 459,271 / 477,429 / 475,939 | 475,939 | 2.08x |

A subsequent three-run measurement, after adding an observation of the intermediate
state to the new generated-move microbenchmark, produced the results below. No
production code changed between these two after measurements. This repeat ran
under heavier host contention; an unrelated test process alone was observed using
about six CPU cores. Even the unchanged evaluation became slower: initial-position
component cost rose from 2,546.7 ns at baseline to 3,275.4 ns in this repeat.
These are shared-host observations, not a controlled estimate of measurement error.

| Workload | Repeat after runs 1 / 2 / 3 | Repeat median |
| --- | --- | ---: |
| Initial perft depth 5 | 51,981,197 / 44,572,778 / 49,487,228 | 49,487,228 |
| Kiwipete perft depth 4 | 53,905,169 / 56,637,341 / 56,297,753 | 56,297,753 |
| Fixed ten-FEN search | 326,016 / 278,849 / 250,996 | 278,849 |

In the repeat, initial/Kiwipete legal generation took 310.6/616.0 ns, public
play plus undo 306.6/654.9 ns, and internal generated play plus undo 18.1/19.7 ns.
The public paired benchmark is unchanged from the baseline harness. The new
internal benchmark additionally exposes the played state to `black_box` before
undo, preventing the pair from hiding the intermediate state from the optimizer.
Evaluation and repeated status generation remain opportunities for later work;
this change does not alter them.

## Identical-behaviour checks

Both debug and release library suites pass: 85 passed, zero failed, five ignored.
The ignored tests are explicit measurement/deeper-perft tests, run separately.
All original rules and search tests remain intact. The 80-row fixture matches
exactly, including nodes, full candidate/PV order and seed-42 replies.

Additional active tests compare more than 8,000 ordered legal lists against the
original copy-make legality filter across seeded games from ten starting FENs.
Every selected move's incremental hash is compared with a full recomputation,
and all games undo to exact initial state. Special-move tests also compare hashes
against fresh FEN imports and verify every root move's undo. All 64 source squares
are checked for each precomputed target table.

The release perft measurement passes all three repetitions at 4,865,609 nodes
for the initial position at depth five and 4,085,603 for Kiwipete at depth four.
Extended ignored checks also pass:

| Position | Depth | Nodes |
| --- | ---: | ---: |
| Initial | 6 | 119,060,324 |
| Kiwipete | 5 | 193,690,690 |
| Position 3 | 6 | 11,030,083 |
| Position 4 | 4 | 422,333 |
| Position 5 | 4 | 2,103,487 |

Formatting, default and all-feature macOS workspace/all-target Clippy, and
all-feature Linux workspace/all-target Clippy through Zig all return rc=0 with
`-D warnings`. No lint allowances, unsafe code, dependencies or protected-path
changes were added. No GPU tests or windows were launched. OCR review was skipped
per Andrew's instruction.

## Reproduction

Run from the worktree root. This shell function applies the required environment
to every cargo invocation and reports its return code:

```sh
set -e
chess_cargo() {
  env -u RUSTUP_TOOLCHAIN -u CARGO -u RUST_RECURSION_COUNT \
    CARGO_INCREMENTAL=0 CARGO_NET_GIT_FETCH_WITH_CLI=true \
    GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0=url.hub-mac-llxprt-branch-1:real-disunity.git.insteadOf \
    GIT_CONFIG_VALUE_0=https://github.com/acoliver/real-disunity.git \
    nice -n 10 cargo "$@"
  rc=$?
  echo "cargo rc=$rc"
  return "$rc"
}
chess_cargo test --release --locked -j4 -p chess3d --lib \
  rules::performance -- --ignored --nocapture --test-threads=1
chess_cargo test --release --locked -j4 -p chess3d --lib \
  fixed_throughput -- --ignored --nocapture
chess_cargo test --release --locked -j4 -p chess3d --lib exact_ladder_baseline
chess_cargo test --locked -j4 -p chess3d --lib
chess_cargo fmt -p chess3d -p xtask -- --check
chess_cargo clippy --locked -j4 --workspace --all-targets -- -D warnings
chess_cargo clippy --locked -j4 --workspace --all-targets --all-features -- -D warnings
CRATE_CC_NO_DEFAULTS=1 \
CC_x86_64_unknown_linux_gnu='/opt/homebrew/bin/zig cc -target x86_64-linux-gnu -fPIC' \
CXX_x86_64_unknown_linux_gnu='/opt/homebrew/bin/zig c++ -target x86_64-linux-gnu -fPIC' \
chess_cargo clippy --locked --workspace --target x86_64-unknown-linux-gnu \
  --all-targets --all-features -- -D warnings
```
