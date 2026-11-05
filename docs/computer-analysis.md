# Computer analysis and opponent policy

[Back to the game README](../README.md).

`chess3d::search` is an original Rust implementation with no external chess-engine
source or dependency. It uses the existing legal rules API without changing the
move generator. Analysis uses iterative-deepening negamax alpha-beta, capture and
promotion quiescence (all legal evasions when checked), TT/MVV-LVA/killer/history
ordering and a caller-sized transposition table. Analysis retains an exact top-N
root set, using its worst score as the alpha bound for later roots. Inferior
bound-only moves are omitted. Reported candidates have comparable exact scores
from the same completed iteration. Neutral analysis keeps one candidate by
default; `Search::candidates(n)` requests more, at a higher node cost.

```rust
use chess3d::rules::ChessState;
use chess3d::search::{Limits, Search, LADDER, choose_move};
use std::sync::atomic::AtomicBool;

let game = ChessState::initial();
let mut search = Search::new(16_384); // fixed TT slots; zero disables caching
let cancel = AtomicBool::new(false);
let analysis = search.analyze(&game, Limits { nodes: 5_000, depth: Some(3) }, &cancel);
assert!(game.legal_moves().contains(&analysis.pv[0]));
let opponent_move = choose_move(&game, &LADDER[3], 42);
assert!(game.legal_moves().contains(&opponent_move));
```

`Score::Centipawns` is relative to the side to move. `Score::Mate` carries an
absolute winner and a ply distance; mate ordering and TT ply adjustment never
convert it to an ordinary centipawn value. The TT validates a fingerprint of the
reversible repetition-history multiset and halfmove counter in addition to the
position hash. This avoids reusing bounds across different draw contexts. Hash
collisions remain theoretically possible. Deeper entries replace shallower ones
within a search age; the next search may replace entries from older ages.

Every visited root, normal and quiescence node counts once. The node limit is
exact, including on interruption. Cancellation is checked before each node, not
inside move generation or evaluation. Only completed iterations are published;
`analyze_progress` can publish them to a host callback. With no completed
iteration, depth zero explicitly denotes an unsearched first-legal-move fallback
and a static score. Completed candidate lists exclude that fallback. Checkmate
precedes draw counters. Supported automatic draws and available threefold or
fifty-move claims score zero; this initial analysis always takes an available
claim rather than analyzing whether to decline it. Normal depth is capped at 64,
and a 128-ply recursion limit aborts an iteration instead of evaluating a checked
king as a quiet position. Rules-counter overflow also aborts the iteration.

`evaluate` exposes independent material, placement, mobility, king-safety and
pawn-structure terms in an `EvaluationBreakdown`, with middlegame/endgame values
and a material phase. The piece-square tables are generated at compile time
from original formulas documented in `src/search/evaluation.rs`. Mobility counts
attack space, not fully legal moves; pawn/king mobility receives no bonus. Pawn
structure includes doubled/isolated penalties and rank-dependent passed-pawn
bonuses. King safety counts shield pawns and enemy attacks in the king ring.
These features are heuristics, not proofs of a tactical explanation.

Opponent policy is separate from neutral analysis. Pip, Rowan, Mira, Ellis, Nia,
Tomas, Imani and Soren have data-only budgets, candidate widths, feature attention,
bounded seeded perturbation and small style biases. Adjacent self-play results
are recorded below; no Elo rating is assigned. A seed reproduces a choice for the same
position, history and profile. Noise applies only to centipawn candidates; it
cannot change mate ordering. No wall-clock timer schedules mistakes.

Before selection, a finite one-reply scan excludes moves that leave a queen
immediately capturable when a queen-safe alternative exists. Policy then selects
within the top-N remaining search candidates, never uniformly over all legal
moves. This conservative rule can reject sound queen sacrifices and does not
prevent every other kind of blunder. If the profile budget completes no iteration,
policy ranks a one-ply static fallback. Safety scanning and fallback preparation
are bounded by the legal move lists, but are outside the analysis node count.
`choose_move` requires at least one legal move and panics on mate/stalemate;
a controller must handle terminal states and draw claims before requesting play.

Run the CPU search regressions or the fixed-budget release throughput measurement:

```sh
CARGO_INCREMENTAL=0 nice -n 10 cargo test --locked -j4 -p chess3d --lib search -- --test-threads=4
CARGO_INCREMENTAL=0 nice -n 10 cargo test --release --locked -j4 -p chess3d --lib \
  search::tests::throughput -- --ignored --nocapture
```

The measurement searches 20,000 nodes each from the initial position and
Kiwipete, using 65,536 TT slots. It prints nodes/second and completed depth; it
is ignored in the default suite. Self-play below measures only this internal pool;
human calibration has not been performed. `BoardDriver` now runs the policy on
the engine's retained CPU queue with one worker and one pending slot. It submits
immutable rules/history snapshots
with a game generation, and derives each move seed from `(game seed, ply)` using
the documented SplitMix-style integer mixing in `retained/computer.rs`. Completed
results are drained on `WorkReady`, checked against both ticket and game generation,
then revalidated and committed through the same single object transaction as human
moves. A computer promotion uses its selected piece without opening a dialog.
Board clicks cannot select or move pieces on the computer's turn; orbit and zoom
remain available. Status names the computer's level/profile on its turn. A
one-shot host timer adds the thinking label after 300 ms of session time, so
faster replies do not flash it. Worker panic,
closed-queue, timer-admission, illegal-result and timeout errors remain visible
until undo/restart.

Undo/restart invalidate the request generation and cancel the engine ticket,
including a result already queued for delivery. Undo removes the human move plus
its computer reply when present, or just the human move while search is pending.
For a black human, the computer's opening is retained as the first decision point;
there is no human move to undo before that point. Available repetition/fifty-move
claims are taken by the computer and shown as a draw without requesting search.

A fixed ten-second emergency cap is separate from each profile's node budget;
the current profiles define no soft move times. Starting a search schedules a
one-shot engine host timer. Its `RetainedEvent::Timer` callback signals the
worker's cooperative stop flag, checked at every node and between policy
candidates. The ticket stays live so `WorkReady` can deliver the seeded policy
choice from the last fully completed iteration. If none completed, status reports
the timeout instead of inventing a move. Normal node-budget exhaustion still uses
the existing finite static fallback. Exact seed reproduction applies when the
emergency cap does not interrupt search. Cancellation cannot interrupt an
individual rules/evaluation call.

Replies, undo/restart, return to title and game end retire both search timers.
The driver cancels retired handles before returning from the current event or
at the next update boundary for direct game actions. The engine delivers
`WorkReady` before due timers, so an already queued reply cancels its timers
without a late stop or status flash. Timer callbacks still run during presentation
pauses. Windowless sessions advance timer time only through `advance_time`;
ordinary polling cannot expire a search. The capture pump's wall-clock stall
watchdog remains outside driver callbacks and independent of the session clock,
so it can detect a stalled test even when session time is frozen.

Windowless timer regressions advance session time across the 300 ms status delay
and ten-second cap, use a controlled slow reply to check completed-move delivery,
and check fast replies, simultaneous work/timer readiness and cancellation on
undo/restart/title/game end. Production submission and timer-admission failures
are also covered.

CPU regressions use real engine queues on GPU-free retained hosts, injecting
completion payloads only for failure/staleness cases. The required windowless GPU
regression uses the production driver and native `WorkReady` delivery from
`frame`/`poll`. It clicks e2-e4 versus Level 1 with game seed 42, verifies b8-c6's
retained identity and transform, captures the reply, and compares exact pixels with an
independent rules-driven reference. `tests/window_smoke.py` adds the same move and
two-ply undo using its existing Xvfb click injection. Do not run a visible window
on the macOS desktop.

## Opponent ladder calibration (2026-11-04)

Measured on an Apple M4 Max (16 logical CPUs), macOS 26.4, Rust 1.97.0.
`examples/calibrate.rs` uses the production policy and only std threads, bounded
to 1–6 workers. The 20 authored UCI lines select short standard openings. Tests
require equal material, no check and an absolute balanced static evaluation below
100 cp; this does not establish theoretical opening equality. Each opening/seed
pair is played with reversed colours. Automatic outcomes and
available draw claims stop play; the 200 searched-ply cap is a draw, with no
material adjudication. Opening plies are excluded from move timings.

The score is wins plus half the draws, divided by games. The 95% Hoeffding
interval uses colour pairs as independent bounded samples, with radius
`sqrt(ln(40) / games)`, clipped to [0, 1]. It describes this fixed opening/seed
pool under that sampling assumption, with no external rating or simultaneous
seven-comparison guarantee. Worker scheduling affects timings only. The CLI
prints every game's index, result and searched UCI moves, plus nodes and timings
in profile order. Pair `i` uses opening `i % 20`, seed `mix(seed + i)`, and move
seed `mix(pair_seed + searched_ply)`; the mixing function is in `selfplay`.

Commands used (the Git URL override is specific to this Mac session):

```sh
unset RUSTUP_TOOLCHAIN CARGO RUST_RECURSION_COUNT
export CARGO_INCREMENTAL=0 CARGO_NET_GIT_FETCH_WITH_CLI=true GIT_CONFIG_COUNT=1
export GIT_CONFIG_KEY_0=url.hub-mac-llxprt-branch-1:real-disunity.git.insteadOf
export GIT_CONFIG_VALUE_0=https://github.com/acoliver/real-disunity.git
nice -n 10 cargo build --release --locked -j4 -p chess3d --example calibrate
nice -n 10 target/release/examples/calibrate timing
nice -n 10 target/release/examples/calibrate stress
nice -n 10 target/release/examples/calibrate ladder 80 6 200 20261104
# A single pair: calibrate LOW HIGH GAMES THREADS MAX_PLIES SEED
# Release-only majority regression, ignored in the default suite:
nice -n 10 cargo test --release --locked -j4 -p chess3d --lib \
  search::selfplay::tests::strongest -- --ignored --nocapture
```

### Adjacent results

Each row has 80 games (40 colour pairs), with W/D/L from the higher level's side.
Every higher level scored at least 73.75%; all seven individual intervals exclude
50%. The seven matches reported 1,008 s of wall time in total.

| Match | Higher-level W/D/L | Score | 95% paired interval | Cap draws |
| --- | ---: | ---: | ---: | ---: |
| 1 vs 2 | 76 / 0 / 4 | 95.00% | 73.53–100.00% | 0 |
| 2 vs 3 | 76 / 0 / 4 | 95.00% | 73.53–100.00% | 0 |
| 3 vs 4 | 68 / 4 / 8 | 87.50% | 66.03–100.00% | 2 |
| 4 vs 5 | 67 / 3 / 10 | 85.62% | 64.15–100.00% | 2 |
| 5 vs 6 | 56 / 6 / 18 | 73.75% | 52.28–95.22% | 2 |
| 6 vs 7 | 63 / 6 / 11 | 82.50% | 61.03–100.00% | 2 |
| 7 vs 8 | 59 / 9 / 12 | 79.38% | 57.90–100.00% | 3 |

### Single-worker move timings

`timing` searches the 20 openings once per profile. `stress` searches the existing
perft positions 2, 4 and 5. Wall time includes queen safety, fallback and all policy
work; node counts include search only. Budgets are nodes per move, with a depth
ceiling of 64 so a low fixed depth cannot hide a budget increase.

| Level | Profile | Nodes | Candidates | Noise ±cp | Opening average / maximum ms | Stress maximum ms |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| 1 | Pip | 80 | 4 | 220 | 0.553 / 0.828 | 0.986 |
| 2 | Rowan | 250 | 3 | 130 | 1.592 / 4.312 | 1.984 |
| 3 | Mira | 750 | 3 | 80 | 4.018 / 5.175 | 6.466 |
| 4 | Ellis | 2,250 | 3 | 45 | 12.305 / 15.570 | 19.741 |
| 5 | Nia | 6,750 | 2 | 24 | 39.052 / 45.461 | 54.793 |
| 6 | Tomas | 20,000 | 2 | 12 | 114.275 / 141.814 | 186.288 |
| 7 | Imani | 60,000 | 2 | 6 | 339.490 / 384.639 | 538.781 |
| 8 | Soren | 135,000 | 1 | 0 | 1,015.774 / 2,107.868 | 989.968 |

Other jobs were running; host load ranged from 35 to 77 during this work.
Level 8's slowest opening move was 2.108 s, slightly above two seconds, while
its average was 1.016 s. These measurements are not a hard wall-time guarantee;
the production ten-second emergency cancellation remains separate from budgets.
In the six-worker 7v8 match, Level 8 averaged 572.930 ms and 134,920 search nodes,
with a 3,264.128 ms maximum. Those positions and scheduling differ from the serial
opening scan. The final sample was reduced from a planned 120 games per pair to 80 because
pilot wall times under host contention projected beyond the calibration limit.

Sampling found repeated legal generation and pawn-list construction in hot paths.
Search now reuses each node's legal list for status and expansion and applies
its generated moves without regenerating legality. Public `play` still validates.
Pawn-file rank masks preserve the original evaluator, checked against its direct
definition over seeded legal games; material-draw detection also avoids a list
allocation. The five existing perft tests and their expected counts are unchanged.

Pip's small budget often uses the static fallback. A regression demonstrates an
elementary beginner opportunity: from the first Italian opening, one fixed seed
plays Bxf7+ and permits Kxf7, losing a bishop for a pawn. Human beginner beatability
and perceived mistake quality still need human play sessions. No Elo is claimed.
The seed-42 computer-reply reference changed from g8-f6 to b8-c6; the retained
identity/transform, independent pixel reference and Linux smoke sequence were
updated, and the full Metal suite passed with the new reference.

Verification: strict default/all-feature macOS Clippy and all-feature Linux Clippy
through Zig passed, as did formatting. CPU library tests passed 91 with 2 ignored;
the full all-feature Metal run passed 160 with 2 ignored. The explicit release
majority regression passed, with Level 8 winning all six games against Level 1.
The Xvfb window smoke was not run on macOS; its Python module passed syntax checks.
