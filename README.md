# rd-chess

An original procedural marble chess scene and position editor, built on the
**Real Disunity** engine. The root Rust package and executable are `chess3d`.
All meshes, marble textures, board geometry, camera presets and position documents
are game-owned. Rendering goes through the engine's `worldrunner` facade; no
engine implementation or third-party 3D art is vendored here.

The scene/position editor accepts arbitrary board layouts in JSON. A separate
CPU-only `rules` module now supplies validated playable chess states, legal moves,
notation and outcomes. The CPU-only `search` module adds analysis and eight
self-play-calibrated opponent profiles, available for computer play. The CLI can apply
legal UCI sequences from the initial game and capture their retained result.
Arbitrary scene documents remain editor inputs, separate from rules authority.
A window without game flags, `--load` or `--view lineup` opens the title menu
over the retained marble board. Choose Play a friend or Play the computer.
Add `--play white|black --level 1..8` to choose your side against a computer:

```sh
cargo run --locked -j4 --features viewer -- --window --view overhead \
  --play white --level 1 --seed 42
```

`--play` requires a window and a level, and rejects editor/lineup/`--moves` inputs.
`--play` starts directly in-game; `--moves ""` starts direct local two-player
play for scripted input. `--seed` controls both marble and the game.
Choosing black lets the computer make the opening move.

## Independent checkout and build

Requirements: Git, Python 3.10+, rustup, and Rust 1.97.0 with Clippy and rustfmt.
The toolchain, edition, minimum supported version and lint thresholds match the
engine. Cargo compilation is bounded to two jobs and uses this repository's own
`target/`, never a source checkout's build directory.

From the rd-chess root:

```sh
# Cargo fetches the immutable engine Git revision automatically.
cargo xtask build
cargo run --locked -j2 -- --help
cargo run --locked -j2 -- --save /tmp/chess-position.json
cargo run --locked -j2 -- --load assets/positions/study.json
```

The `worldrunner` facade is a Git dependency pinned to the full commit
`5f99b20fb3b084de9d33d80e214a1fc7bc636e12` from
`https://github.com/acoliver/real-disunity.git`. Cargo builds that revision,
not a sibling checkout. `engine-revision` records the same SHA, and the default
and viewer dependency graph gates verify every SDK package's exact Git origin,
requested revision and resolved checksum, with one rendering implementation.
When updating the engine, change the Cargo `rev`, `engine-revision` and lockfile
together.

For optional SDK inspection, `python3 tools/bootstrap_engine.py` fetches the pinned
revision into `../real-disunity`. It stages a clone and publishes it only after a
successful checkout and facade check. Existing sibling checkouts are read-only:
dirty, wrong-revision or non-engine directories fail without reset or checkout.
`--revision` or `ENGINE_REVISION` selects an inspection checkout only; it does
**not** override Cargo's immutable dependency. No sibling is required to build,
run or validate the game. `cargo xtask bootstrap` performs the same optional check.

HTTPS access to a private engine repository requires credentials. For the optional
bootstrap, `ENGINE_READ_TOKEN` supplies read-only authentication without persisting
it in Git config. Cargo uses its standard Git authentication for the pinned dependency. CI uses the `ENGINE_READ_TOKEN` repository secret when supplied,
otherwise its own workflow token. A workflow token does not grant access to a
separate private repository by itself.

`Cargo.lock` belongs to this game workspace and is checked in. Validation uses
`--locked`; after intentionally changing the engine or dependencies, regenerate
it explicitly with `cargo generate-lockfile` and review the change. No source
monorepo, copied history, generated evidence or unrelated game assets are needed.

## Rendering and position documents

```sh
# Select an installed Vulkan ICD explicitly (the filename varies by system).
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json
cargo run --locked -j2 -- --capture /tmp/chess-hero.png \
  --view hero --width 1400 --height 1050
cargo run --locked -j2 -- --load assets/positions/study.json \
  --save /tmp/study-roundtrip.json --capture /tmp/chess-study.png

# Windowing is opt-in both at build time and launch time.
cargo run --locked -j2 --features viewer -- --window --view hero

# Windowless retained test control: legal moves, resident assets, no visible window.
cargo run --locked -j2 --features viewer -- \
  --moves "e2e4 e7e5 g1f3" --capture /tmp/chess-moves.png
```

The default `scene` engine dependency excludes the window feature. The game's
`viewer` feature enables `worldrunner/scene-viewer`, including request-driven
windowless capture. Hero, overhead and wide
lineup views share the same prepared scene and directional-shadow settings for
offscreen and window rendering. Original curved sculpts and deterministic
seeded marble remain procedural source assets, not copied image textures.

- `--seed <u64>` controls marble veining, including the high seed bits.
- `--load` and `--save` read/write version-1 `chess3d-position` JSON. Seed/style,
  stable piece IDs, kinds, sides and current squares round-trip. `assets/positions/`
  contains the initial layout and a noninitial study. Edit the JSON to move,
  remove or promote pieces; at most 32 pieces, unique IDs/squares and board bounds
  are validated. Empty boards are accepted. No move legality is implied.
- `--seed` with `--load` is rejected. Image dimensions are bounded to 64–4096 per
  axis. Captures cannot alias position inputs/outputs. Position and PNG writes
  use atomic replacement; if save and render are requested, save completes first
  and the two files are not a single transaction.
- Inspection/save/load need no GPU. Captures require a functioning graphics
  adapter; Mesa lavapipe is a supported software Vulkan device.
- Left click selects a side-to-move piece and highlights its legal targets.
  Click a target to move, the selected piece or outside the board to deselect,
  or another friendly piece to switch selection. An illegal board target does
  nothing. A press moving beyond six physical pixels orbits instead of clicking;
  wheel zooms and Home restores the preset. U/Backspace undoes one ply in local
  play, or returns to the last human decision against a computer. R restarts
  the game and resets the camera. Escape cancels selection/promotion first, then
  opens the in-game menu. Escape again resumes. Loaded editor and lineup views retain R/Home reset and Escape exit.
  Resize and unmap/remap affect presentation only; camera changes never rewrite
  the saved position. `--capture` and `--window` cannot be combined.

## Retained board and game test controls

`--moves "e2e4 e7e5 g1f3"` applies UCI moves in order through `ChessState`.
It starts from the legal initial game, rejects illegal notation or moves with the
failing token, and validates the whole sequence before any save or capture write.
It cannot be combined with arbitrary editor `--load` input or the asset lineup.
Without capture it needs no GPU; `--save` records the resulting position document.
Move capture requires `--features viewer`. Default-feature editor captures still
use the facade's direct scene capture API and have no viewer dependency.

`retained::RetainedBoard` maps rules piece IDs to retained body/eye `ObjectId`s.
Captured IDs stay reserved for undo. Pawns reserve an eye ID so promotion to a
knight needs no identity reassignment. The initial scene builds all six meshes and
marble textures once. Play and undo produce object descriptors only, referencing
those resident mesh/material tables. Capture and castling each publish one
`RetainedUpdate::Objects` transaction, including all affected bodies and eyes.
Promotion replaces the body mesh/material reference and adds the reserved eye
when needed. No mesh or texture generator is called during these edits.

The native `BoardDriver` owns the rules board and identity mapping throughout
play and `--moves` captures. Editor scenes have no rules game. The driver submits
one queued move transaction per update. A `ViewerError::BackPressure` leaves it
at the queue head; other errors end the run. Desktop viewing uses
`RetainedHost::run`; captures use the same driver's windowless session. The bounded
capture test pump drains driver changes before requesting the accepted generation.
It never runs from an event-loop callback.

The capture pump and windowless computer-reply wait use `retained::ProgressPump`.
Accepted-generation changes, preparation-count changes, submitted ordinary frames,
and capture acceptance/completion reset its 180-second stall clock. Calling `frame`
without submitting or polling pending work does not count as progress. The stall
window allows a single pipeline compile on a heavily loaded software-GPU host:
gate result `071fb52630bf` exceeded the former 60-second capture limit while the
same suites grew from 25-36 seconds passing to 81-127 seconds under load.
A separate 1,200-second total bound catches endless progress, allowing over 30
times the passing suite duration. Neither bound restarts between preparation
and capture. Failures report the awaited operation, unchanged progress values,
and stall/total elapsed time. Checks run between engine calls; they cannot
interrupt synchronous engine work. CPU-only computer test waits share the same
watchdog, observing whether a result remains pending. Production computer search
budgets are unchanged. Fake-clock unit tests cover slow progress, stalls, each
progress signal, and the independent hard bound without sleeping or a GPU.

Board clicks query the engine's last presented geometry at physical top-left
pixels. The filter includes piece bodies, knight eyes, the board and visible
target rings, excluding the studio floor and decorative frame/inlay. Body and eye
hits resolve to that piece's current rules square, including tall silhouettes in
hero view. Board and ring hits resolve from their world intersection. Only a
geometry miss falls back to the camera ray's Z=0.05 m board-plane intersection,
so off-board clicks still deselect. Overhead remains the readable play preset
(`--view overhead`). Input wiring calls the engine's `OrbitCamera`, rather than
implementing camera math in chess. Highlights share one resident ring mesh and
reserved per-square object identities. A move removes its highlights in the same
object batch as the pieces.

Clicks are discarded, not queued, while a game transaction is pending or the
accepted generation has not been presented. The driver records presentation
notifications (completed captures for its windowless mode) and also requires a
hit's generation to equal the accepted generation. A stale hit is a no-op, not a
plane fallback. This prevents both piece targeting and off-board deselection
against stale geometry. Orbit and zoom remain available during presentation.

One native UI world owns status text and the promotion choices. `UiRouter` routes
four choice controls under a modal root before board input; its consumed flag
prevents a choice click from reaching the board. Escape cancels promotion without
committing a move. Text is extracted through the engine and submitted with
`RetainedUpdate::Ui` only when status, dialog visibility or viewport changes.
The bundled unmodified Space Mono font is licensed under SIL OFL 1.1; its notice
and license are in `assets/fonts/OFL.txt`. No game-owned text renderer is used.
Status names check, mate, stalemate, automatic draw reasons and available
threefold/fifty-move claims. Claim draw appears only when a human can claim the
current position. Claims and resignation record a controller result and stop play;
fivefold repetition and seventy-five moves remain automatic rules results.

With the viewer feature, `tests/retained.rs` checks exact object edit sets,
identity-preserving undo, resource-table storage reuse and admission pressure.
`tests/retained_gpu.rs` requires a device, applies a sequence with castling and a
capture, captures the changed frame, undoes every move and requires exact initial
pixel equality and restored object identities. Missing devices fail even when
`CHESS3D_REQUIRE_GPU` is unset. On macOS run GPU tests with `WGPU_BACKEND=metal`
through the shared rd-stranded AquaJob launcher, not a visible window.
`tests/interaction.rs` drives synthetic retained pointer/key observations through
selection, illegal clicks, orbit cancellation, promotion, undo/restart and status.
`tests/interaction_gpu.rs` plays synthetic clicks through the windowless driver,
asserting status/dialog pixel regions and changed board pixels. It also checks
that unchanged status is not resubmitted and a move uses one object batch plus
one status update. The Xvfb smoke retains orbit/reset/resize and adds a click
move compared to an exact rules-driven reference, followed by undo. That
Linux real-window smoke is run by the gate, not on the macOS desktop.

## Play menus

Title, computer setup, pause and game-over screens share the existing native UI
world and retained overlay compositor. The game owns labels, profile descriptions,
menu structure and typed actions. Engine list selection owns pointer targeting,
keyboard focus, Up/Down/Home/End navigation, scrolling and state tints. Arrows
select command rows without executing them; Enter/Space or a click executes them.
Tab moves between setup fields. Side and level selection takes effect immediately.

Computer setup offers White, Black or seed-determined Random, and all eight named
profiles with one-line descriptions in a clipped scroll list. The defaults are
White and Level 1. Start begins a new game; Back or Escape returns to title.
Choosing Black schedules the computer's opening through the existing work queue.
The adjacent self-play ordering and its limits are documented below. These
profiles have no measured external ratings.

The in-game menu offers Resume, Undo, Restart, Resign and Return to title. Undo is
disabled before a human decision exists. Menus consume board input, including
clicks outside their panels. Resignation names the winner. Return to title bumps
the game generation, cancels outstanding work, removes the computer controller
and resets the board without rebuilding its resident resources. Terminal games
show the result and reason in status and a New game/Title modal. New game keeps
the current opponent configuration; Title returns to the title choices.

`tests/menus.rs` exercises synthetic keyboard and pointer transitions, draw claims
and terminal actions without a GPU. Computer unit tests exercise a black-side
opening and cancellation on title return using real engine queues. The required
`tests/menus_gpu.rs` captures title and in-game menus through the same windowless
path, checks exact stable repeats, unchanged board pixels outside the panel and
visible focus tints that move with navigation. No GPU test skips a missing device.
The Xvfb smoke adds a no-game-flags title launch followed by Enter on Play a friend.
It is not run as a visible macOS window.

Settings controls and game save slots are not included in this engine pin.
P6 part 2 needs native sliders/toggles and the engine's bounded transactional
application settings/save-slot service, including application directories, listing,
metadata and explicit corruption/permission errors. Chess will own versioned game
payloads containing the initial FEN, move history, seed, opponent and controller
result, so castling rights and repetition history survive relaunch. Fixed slot
labels avoid requiring text entry; editable labels and cross-display UI scaling
need the separate retained text-input/display-scale APIs.

## Legal chess rules

`chess3d::rules::ChessState` owns a private 0x88 board, turn, castling rights,
en-passant target, move counters and Zobrist repetition history. It starts from
the standard initial position or a validated six-field FEN. It never infers game
state from an editor `Position`, which may have no kings or other illegal layouts.

```rust
use chess3d::rules::ChessState;

let mut game = ChessState::initial();
let chess_move = game.parse_uci("e2e4")?;
assert_eq!(game.to_san(chess_move)?, "e4");
game.play(chess_move)?;
let scene_document = game.to_position(42);
game.undo()?;
# Ok::<(), chess3d::rules::MoveError>(())
```

Legal moves include both castles, en passant checked for exposed king attacks,
and all four promotions. `play` revalidates moves and commits atomically; `undo`
restores pieces, identities, rights, counters, targets and repetition history.
SAN parsing accepts exact standard SAN, including check/mate suffixes; UCI accepts
lower-case promotion suffixes. Scene conversion preserves piece IDs across moves
and promotions within a state, but importing a fresh FEN assigns fresh IDs.

`status` gives mate/stalemate and automatic draws. Fifty-move and threefold draws
are claimable, exposed separately through `draw_claims` and `claims_after` for an
intended move. Fivefold repetition and seventy-five moves are automatic, with
checkmate precedence. The game controller must stop play on terminal status and
record claims, resignations and agreed draws; the rules value remains usable for
analysis. FEN restores placement and counters, not repetition history. Saving a
game requires its initial FEN and moves.

Automatic dead-position recognition covers bare kings, one bishop or knight
against a bare king, and bishop-only positions with all bishops on one square
colour. Other dead positions, including blocked pawn fortresses, are not detected.
FEN validation checks local legality and promotion budgets, not full retrograde
reachability. Move counters use `u32`; an overflowing move fails without mutation.
Timed play, flag-fall, Chess960 and PGN are outside this rules-core proposal.

Run the CPU-only rules suite without a graphics adapter:

```sh
CARGO_INCREMENTAL=0 nice -n 10 cargo test --locked -j4 -p chess3d --lib rules -- --test-threads=4
```

The default suite includes initial-position perft through depth four and the
Chess Programming Wiki's positions 2–5, with their verified FENs. It also covers
special moves, draw claims/history, strict FEN errors, SAN/UCI round trips, exact
undo and deterministic random legal sequences. No perft test is ignored.

## Computer analysis and opponent policy

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
remain available. Status names the level/profile while thinking and displays
worker panic, closed-queue, illegal-result and timeout errors until undo/restart.

Undo/restart invalidate the request generation and cancel the engine ticket,
including a result already queued for delivery. Undo removes the human move plus
its computer reply when present, or just the human move while search is pending.
For a black human, the computer's opening is retained as the first decision point;
there is no human move to undo before that point. Available repetition/fifty-move
claims are taken by the computer and shown as a draw without requesting search.

A fixed ten-second elapsed safety budget is separate from each profile's node
budget. Retained updates signal cooperative cancellation at the deadline. The
policy also checks that deadline on its worker at every node and between policy
candidates, so paused presentation does not leave search running indefinitely.
It returns the seeded policy choice from the last fully completed iteration; if
none completed, status reports the timeout instead of inventing a move. Normal
node-budget exhaustion still uses the existing finite static fallback. Exact seed
reproduction applies when the elapsed emergency cap does not interrupt search.
Cancellation cannot interrupt an individual rules/evaluation call.

Engine limitation: this pin has work wakeups and retained ticks, but no scheduled
host timer service. The cap uses monotonic deadlines in those existing callbacks;
there is no game-created timer thread or executor. A literal scheduled host timer
requires an engine API before the game can adopt it (engine issue `c997bf5e36af`).

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

## Validation

```sh
cargo xtask python-tests
cargo xtask check                    # fmt + strict default/viewer Clippy
export VK_DRIVER_FILES=/usr/share/vulkan/icd.d/lvp_icd.json
cargo xtask test                     # all targets with viewer; GPU required
cargo xtask acceptance --output artifacts/offscreen-001
cargo xtask window --output artifacts/window-001
# Or run every gate, including both rendering harnesses:
cargo xtask ci --output artifacts/ci-001
```

Output directories must be new/empty; previous evidence is not overwritten.
Use `CARGO_TARGET_DIR` to relocate build artifacts; the offscreen/window harnesses
also accept `--target-dir`. Keep evidence and target directories separate.

The migrated stdlib-only offscreen harness drives the real binary, not a fake
renderer. It records engine/game checkout revisions and dirty state, toolchain,
Vulkan adapter, ICD/binary hashes, command logs and decoded image measurements.
It checks:

- required-device Rust acceptance and nonblank, opaque, correctly sized images;
- exact same-device decoded-pixel repeats for hero, overhead and lineup;
- exact initial/study/edited save/load round-trips and different edited images;
- high seed bits changing rendered texture, plus projected board tile parity;
- invalid position/dimensions and a deliberately absent adapter returning errors,
  preserving existing files and not creating failed captures;
- the required Rust GPU test itself **failing** with a missing ICD.

`CHESS3D_REQUIRE_GPU=1` is set by the harness/xtask/CI. The game GPU test is
unconditionally required even without that environment variable: missing adapters
are failures, never skips. Python decoder/process/timeout/bootstrap tests are
supporting controls, not substitutes for observed GPU acceptance. Pixel equality
is required only on the same device/driver; no cross-GPU golden-image claim is made.

The original `tests/window_smoke.py` is retained, with its study fixture naturally
resolved from this standalone root. `cargo xtask window` builds the viewer and
launches it using an exclusive Xvfb `:100` via `xvfb-run`; it never uses the desktop.
Requirements: Xvfb, xvfb-run, xauth, xdotool, libX11 and Pillow. Missing prerequisites
fail explicitly. The smoke compares real window pixels to fresh offscreen frames,
exercises orbit/zoom/reset/resize/unmap/remap, checks saved positions, title-to-friend Enter navigation, Escape menu/resume and
clean window-manager exits. Its original readiness checks are bounded; the
wrapper bounds the whole window run to five minutes and cleans up its process group.

The GitHub Actions workflow fetches the independent engine, provisions Mesa
lavapipe and window-test prerequisites on the runner, and invokes `cargo xtask ci`.
Both GPU and Xvfb acceptance are mandatory, with logs/PNGs uploaded even on failure.
Manual runs accept an explicit `engine_revision` for the optional inspection
checkout only; the compiled SDK remains the Cargo-pinned revision. Nothing in the local commands
installs system packages, changes remote settings, commits or pushes.

## Limits

The scene uses albedo marble, dielectric surfaces, metallic rails and a fixed
studio sun with a PCF directional shadow map. It is stylized procedural art,
not measured stone or a complete simulation. Unusual aspect ratios or extreme
orbit/zoom can crop the composition. Generated acceptance captures are local/CI
artifacts only and are not bundled as source assets or historical evidence.
