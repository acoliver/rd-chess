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
`0671eedd43021075f7069ea9a43b148fbd634d18` from
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

The title lists Continue (when available), Play a friend, Play the computer,
Lessons, Puzzles, Saved games, Review games, Settings and Quit, in that order. Saved games
includes Import PGN; pause and game-over screens offer Export PGN.

Title, computer setup, pause and game-over screens share the existing native UI
world and retained overlay compositor. The game owns labels, profile descriptions,
menu structure and typed actions. Engine list selection owns pointer targeting,
keyboard focus, Up/Down/Home/End navigation, scrolling and state tints. Arrows
select command rows without executing them; Enter/Space or a click executes them.
Tab moves between setup fields. Side and level selection takes effect immediately.

Computer setup offers White, Black or seed-determined Random, and all eight named
profiles with one-line descriptions in a clipped scroll list. The initial defaults
are White and Level 1; title settings can change them. The setup summary shows
the active side and level. Start begins a new game; Back or Escape returns to title.
Choosing Black schedules the computer's opening through the existing work queue.
The [self-play ordering and its limits](docs/computer-analysis.md#opponent-ladder-calibration-2026-11-04)
are documented separately. These profiles have no measured external ratings.

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

## Saved games and settings

The viewer stores games and settings through the engine's `ApplicationStore`.
The default root is `application_root("org.acoliver.chess3d")`, for example
`~/Library/Application Support/org.acoliver.chess3d` on macOS. `--data-dir PATH`
overrides that root. Windowless captures and editor views do not open the store.
Tests use temporary directories and never write player data.

Every committed human or computer move, undo and restart atomically saves the
`current` slot. Resignation and claimed draws also save their result. Returning
to title resets the displayed board without overwriting that slot. Continue is
shown only for a valid unfinished current game. Saved games on the title and
Save / Load in the pause menu offer three numbered slots with generated labels,
such as `vs Level 3, move 24`. Saving from the title copies the current game,
including a finished result, rather than the title's background board.

Version-1 chess payloads contain an initial FEN, ordered UCI moves, seed,
two-player or computer mode, human side, level and an optional result. Loading
validates the whole payload and replays each move through `ChessState` before
changing the live board. Castling rights, en passant, counters, piece identities
and repetition history therefore survive relaunch. Computer games retain their
side and level and schedule a reply through the existing work queue when needed.
Loading may replace seed-dependent resident resources through the engine; normal
moves, undo and restart continue to use object-only updates.

Settings on the title offers native list choices for default computer level and
human side (White, Black or seeded Random), legal-move markers and Hero/Overhead
camera. Choices apply immediately and persist through the engine's typed Settings
format. An explicit `--view` overrides the saved camera at launch. Sliders and
toggles are not used. Editable slot names still require native text input.

Storage reads and writes are bounded to 1 MiB per game, 256-byte labels and 4 KiB
settings documents. Corruption, illegal saved moves, unknown versions, oversized
files and I/O failures appear in status without stopping play. An invalid
existing slot or settings document is not overwritten, including by autosave;
move or remove the damaged file to restore saving. Failed loads leave the live
game unchanged. File replacement and OS permission handling remain engine-owned.

`tests/saved_games.rs`, `tests/storage_errors.rs` and `tests/saved_menus.rs`
cover replay, autosave, slot labels, Continue, settings, results and preservation
of invalid or unwritable files. Required windowless Metal/lavapipe tests relaunch
a new driver after three moves, activate Continue and require exact pre-relaunch
pixels, including restoration from a different starting seed. A second test
requires the resumed computer's native WorkReady reply. The isolated Xvfb smoke
uses temporary `--data-dir` roots and checks relaunch/Continue before undo.

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
Timed play, flag-fall and Chess960 are outside this rules core.

Run the CPU-only rules suite without a graphics adapter:

```sh
CARGO_INCREMENTAL=0 nice -n 10 cargo test --locked -j4 -p chess3d --lib rules -- --test-threads=4
```

The default suite includes initial-position perft through depth four and the
Chess Programming Wiki's positions 2–5, with their verified FENs. It also covers
special moves, draw claims/history, strict FEN errors, SAN/UCI round trips, exact
undo and deterministic random legal sequences. Ignored release benchmarks add
initial-position perft through depth six and Kiwipete through depth five.
[Move-generation performance](docs/movegen-performance.md) records the baseline,
techniques, reproduction commands and exact-behaviour regression coverage.

## PGN records

`rules::pgn` imports and exports game records without a GPU or viewer feature.
`PgnGame` owns the initial FEN, validated main-line moves, current rules state,
tags, result and optional comments. Start a record from a rules position, apply
legal moves, and export it:

```rust
use chess3d::rules::{ChessState, pgn::{PgnGame, parse}};

let mut record = PgnGame::new(&ChessState::initial());
let chess_move = record.state().parse_uci("e2e4")?;
record.play(chess_move)?;
record.tags.insert("Black".into(), "Computer (Level 2)".into());
record.set_comment(1, "White occupies the center.")?;
let pgn = record.export()?;
let imported = parse(&pgn)?;
assert_eq!(imported[0].state(), record.state());
# Ok::<(), Box<dyn std::error::Error>>(())
```

Export supplies the Seven Tag Roster with defaults, derives SetUp/FEN tags for
noninitial starts, writes standard SAN and move numbers (including a black-side
start), and wraps movetext at 80 columns. Player names are metadata supplied by
the caller. Comments attach to ply zero or follow a committed ply; whitespace
is normalized. Comments containing braces use semicolon lines. An unbroken
comment word over 78 columns produces a typed export error instead of being
altered or exceeding the line limit. Tags escape quotes and backslashes.

Import accepts multiple games, optional SAN check/mate suffixes, zero-spelled
castles, promotions with or without `=`, comments, annotations and recursive
variations. Variation moves are validated from their branch position but are
not retained. The main line rebuilds repetition history. Results are recorded
declarations, so resignations and agreed draws need no terminal board position;
the Result tag must agree with the movetext marker. Input is limited to 4 MiB
and variation nesting to 32. Errors name the one-based game, ply and token.

`cargo run --locked -j4 -- --pgn-check games.pgn` validates the entire file before
printing `Game N: <final FEN> <result>` for each game. This CPU-only mode accepts
only `--pgn-check`; it rejects review, explanation, lesson, puzzle, scene, save,
play and `--data-dir` options,
including explicit rendering defaults, and never opens a window.

### PGN menu files

Export PGN is available in pause and game-over menus. Saved games offers Import
PGN from the application-data `exports/` directory, including `--data-dir` roots.
Imports replay the main line, result and undo history as a local two-player game.
See [PGN menu files](docs/pgn-menu-files.md) for metadata, file naming, controls,
input limits, path safety and tests.

## Computer analysis and opponent policy

See [computer analysis and ladder calibration](docs/computer-analysis.md) for
search APIs, opponent budgets, host-timer scheduling, cancellation and measured
self-play results. The faster rules generator and internal make/unmake path
preserve the search tree; see [move-generation performance](docs/movegen-performance.md).
The frozen pre-calibration ladder fixture is checked without changing its recorded
profiles or expected rows:

```sh
cargo test --release --locked -j4 -p chess3d --lib exact_ladder_baseline
```

## Verified move explanations

`chess3d::teaching::explain` supplies CPU-only explanations for human moves and computer
replies. `explain_move(&state, mv, budget)` and `explain_reply` return the same
`Result<Explanation, MoveError>`. They revalidate the move, preserve the input
state and are deterministic for a given position/history and budget. No model,
network service or new dependency is involved. The game menu's Why? control reuses
these learner sentences after a computer reply.

```rust
use chess3d::rules::ChessState;
use chess3d::teaching::{Budget, explain_move, san_line};

let state = ChessState::initial();
let mv = state.parse_uci("g1f3")?;
let explanation = explain_move(&state, mv, Budget { nodes: 100_000, depth: 3 })?;
for fact in &explanation.facts {
    println!("{}: {}", fact.kind.label(), san_line(&state, &fact.line));
}
# Ok::<(), chess3d::rules::MoveError>(())
```

For a manual check without a graphics adapter:

```sh
cargo run --locked -j4 -- --explain '7k/8/5KQ1/8/8/8/8/8 w - - 0 1' g6g7
```

`--explain <fen> <uci>` prints learner sentences, their SAN evidence and the
principal line without preparing a scene or opening a window. The optional
`--explain-verbose` flag adds developer diagnostics from `Explanation.limits`:
node counts, depths, caps and exchange scope. No other options may accompany
`--explain`. The evidence for every fact starts in the supplied position.
Missed opportunities start with the alternative move; other facts start with
the played move. `sentences` follows the selected `facts` in order, then appends
any neutral sentence and at most one short caveat.

Presentation precedence is mate > winning tactic (fork/pin/skewer/discovered) >
hanging/exchange > threat > check > castling/promotion notes. This orders the
facts without discarding independent information. Subsumption requires the same
evidence line or a direct-move prefix: mate covers that line, a winning motif
covers its material threat, and a king fork or discovered check covers the
standalone check. Alternatives with a different first move remain independent.
Captures and promotion notes describe distinct board changes and are retained
unless mate covers them. Each retained fact keeps its original verification line.

Material sentences name a piece only when the line gains exactly that captured
piece without losing material or promoting. Other gains and losses use net pawn
values rounded to at most one decimal, for example `wins about 2.2 pawns` for a
knight captured at the cost of a pawn. Numeric material fields retain their
original units for programs; learner sentences never use centipawns.

Supported facts are captures with actual victim squares and material changes
(including en passant and capture promotions), check/mate, hanging pieces,
winning/losing/equal exchanges, forks, exploitable absolute/relative pins,
skewers, discovered attacks/checks, promotions, castling, lost castling rights,
mate-in-one/material threats and missed mating/material alternatives. Geometry
only proposes motifs. Material motifs additionally require a legal continuation
with a positive material change after the move, not just a positional score or
an attack pattern. Castling-rights facts make no claim about improved king safety.

One node allowance covers all phases. The explained position is admitted first;
one quarter of the remaining allowance ranks all legal alternatives using the
existing neutral search. Legal static-exchange trees and a separate material-only
alpha-beta verifier use the remainder. Material accounting uses fixed values of
100/320/335/500/950 centipawns for pawn/knight/bishop/rook/queen. Mate ordering
remains separate from centipawns. Capture changes and exchange gains use the
mover's perspective; hanging-piece gains use the named capturer's perspective.
Verification searches all legal replies at normal depth and extends captures,
promotions and check evasions. Requested
normal depth includes the played move and is capped at six. Verification and
exchange continuations are capped at that depth plus eight plies. Alternative
ranking retains the existing search's separate 128-ply recursion cap. Every
visited analysis/exchange/verification node counts against the shared allowance;
legal move generation, validation, notation and geometric queries are not nodes.

Exchange evaluation considers every actual legal attacker and recapturer on one
square, updating x-rays, pins, promotion and king safety through `ChessState`.
Either side can stop exchanging, except when every legal check evasion recaptures
on that square. Off-square tactics are outside SEE, so a hanging-piece sentence
reports its exchange scope rather than claiming a forced game result. The general
verifier also considers off-square moves within its normal depth.

These are bounded lines, not game-theoretic proofs. The verifier stops at draws
and available draw claims like the existing search, but retains the actual
material change there. Winning a rook can leave a drawn king-and-knight ending;
a material fact does not imply a winning game. It omits material motifs whose follow-up
cannot be demonstrated, even if the pattern exists. Only the moved piece is
considered as a fork attacker. Defensive geometric maps can conservatively omit
a fork when a defender is pinned. Threat detection requires a verified next-move
continuation and omits promotion setups; it does not catalog threats that the
opponent can fully neutralize. A missed opportunity must be the neutral search's
best alternative and both played/alternative verification must finish. Deeper
combinations, strategic explanations and complete motif enumeration are not
promised.

A score-only alternative requires comparable centipawn scores from a completed
search iteration and an improvement of more than 100 centipawns (one pawn).
At the default depth-three, 100,000-node shared budget, smaller placement-score
differences are too sensitive to the search horizon to justify criticism. This
is a conservative teaching margin, not a calibrated estimate of search error.
Verified missed material or mate also permits a named alternative, without a
centipawn threshold. Other mate-score differences alone do not trigger a claim.
The sentence names the move: `The search prefers e3: this line scores better in
the current search.` The CLI labels both continuations, for example `Line after
e3: ...` and `Line after Nf3: ...`. `Explanation.line` always begins with the
played move; `alternative_line` holds the named preference when present. Below
the threshold, a resolved position without a tactical fact says `No tactical
issue found within the analysis limits.` If no completed comparison exists,
the score claim is omitted. A nonterminal material line or search preference
adds a short horizon caveat, for example `Checked 3 moves ahead.` Here moves
counts individual turns (plies), including the supplied move, not move pairs.
Terminal material lines and direct facts do not need that caveat. Incomplete
analysis instead reports `The search could not settle this line.` It never
assigns a confident blunder label. Detailed limits remain available through
`limits.diagnostics()` and `--explain-verbose`. CPU regressions in
`tests/teaching_explain.rs` use original hand-built positions, exact typed facts/templates,
legal evidence replay, material/mate premises, negative motifs, deterministic
budgets and special moves. `tests/cli.rs` checks the CLI with graphics disabled.

## Offline course and puzzles

`teaching::course` embeds a versioned first course with 12 original lessons and 19 board
tasks, plus 300 CC0 Lichess puzzles. Both work without a viewer or network access.
The course covers piece movement, check, basic mates, hanging pieces and counting,
forks, pins, skewers, opening principles and mixed practice. It validates all
prerequisites, positions and solution branches before a runner can use them.

```sh
cargo run --locked -p chess3d -- --lesson rules.first-moves
cargo run --locked -p chess3d -- --puzzle 00008
cargo test --locked -p chess3d --lib --test teaching_course --test teaching_explain --test puzzles --test cli
```

`--lesson ID` prints the steps, FENs, hints and accepted lines after validating
the course. `--puzzle ID` prints the position after the opponent's setup move,
then the solver's line. These inspection modes cannot combine with other options, write position files or
launch a window. The runners handle wrong-answer feedback, retries, progressive
hints, scripted replies and alternative mates through the existing rules core.
The window's Lessons and Puzzles title choices open native browsers and playable
board tasks, with hints, retries, scripted replies and completion marks. The
game menu's Why? opens learner sentences after a computer reply. Lesson and
puzzle progress loads at startup and persists when changed through a separate,
versioned `training-progress` record in the saved-game `ApplicationStore`.
Corrupt progress produces a typed error without disabling game saves or Continue.
See the course documentation below for controls, storage limits and teaching-screen
verification.

Lesson steps, task feedback, hints, Why? and review explanations use the engine's
styled paragraph flow. Lesson headings are gold and SAN move names from the rules
state or review evidence are cyan; plain prose keeps its normal colour. Authored
newlines and blank paragraphs survive wrapping. The engine measures text at the
resolved panel width, rather than inserting character-count line breaks.

Course-authored `terms`, lesson IDs and lesson titles become underlined links in
lesson and review paragraphs. A link opens the teaching lesson; locked targets
show their unfinished prerequisites without changing the board. Tab and Shift-Tab
visit individual links, and Enter or Space activates the focused link. Pointer
activation uses the engine's glyph hit regions. Review SAN links show the position
after the named move, including searched replies and alternatives; the original
record stays unchanged. At engine revision `42d1ac5c`, native accessibility exposes
paragraph text but has no per-span link role or link action identity.

Review moments and details retain their existing wheel scrolling and size the
scroll content from the engine's full text height. The study/Why? panel has no
text scrolling or paging control. When its measured content exceeds the fixed
height, it shows the fitting prefix followed by `...` and prints a truncation
notice to stderr. Resizing measures the complete source again, so a wider panel
can restore omitted text. The engine has no automatic ellipsis and accepts no
U+2026 ellipsis character, so the game uses three ASCII periods. Engine spans
provide colour only, without bold, italic or mixed font sizes within a paragraph.
CPU tests cover ordered spans, authored breaks, line and glyph bounds, resize
recovery and scrolling to the final review line at 800x600 and 1200x900. Metal
reading and rendering checks remain separate.

Native menu and study panels use a shared alpha of 1.0 so the board cannot show
through the text background. Locked lessons retain their `[locked]` marker and
use a lighter grey tint. Disabled Next reads `Next [off]`; enabled Next reads
`Next` and uses the enabled control colours. The status line appears on board
screens, not on the title, setup or lesson/puzzle browsers. Unit tests calculate
WCAG contrast from the actual display-linear panel and control colours: locked
entries and enabled controls require 4.5:1, disabled controls require 3:1.

See [the course format and runner contract](assets/lessons/README.md) and
[the puzzle notice, CC0 licence statement and subset reproduction](assets/puzzles/README.md).
The puzzle subset is 54,576 bytes, with 50 selections per target theme and
three rating bands. Lichess's CSV FEN is before the first move; tests explicitly
apply that opponent setup move before the learner starts. Original course prose
and game source retain the repository licence; CC0 applies to the puzzle data.

## Own-game review core (P10)

`chess3d::review::Review` analyzes an initial `ChessState` and its committed legal
move record. Supply the player's side explicitly. The record may end at mate,
automatic draw, resignation, agreement or a claim; the caller owns the recorded
result. Prefixes are also accepted for inspection. Construction replays the whole
record and rejects illegal moves or moves after an automatic outcome before
analysis starts. Review never changes the supplied state or original result.

The CPU-only manual interface does not require the viewer or a graphics device:

```sh
cargo run --locked -j4 -- --review-moves "g1f3 g8f6 b1c3 b8c6"
cargo run --locked -j4 -- --review-moves "d1d4 a4d4" \
  --review-fen "4k3/8/8/8/r7/8/8/3QK3 w - - 0 1"
```

The table covers both sides; CLI teachable moments belong to the initial side to
move (white by default, black for a black-to-move FEN). The library accepts either
player independently. `--review-fen` requires `--review-moves`. Review cannot be
combined with game/editor/rendering actions or `--explain`, `--lesson`, or `--puzzle`, `--pgn-check` or `--data-dir`.
The six CPU inspection modes, including `--pgn-check` and `--inspect-title`, are mutually exclusive;
explicitly supplied rendering defaults such as `--view hero` are still rendering options.
`--inspect-title` requires the viewer feature but no GPU or window. Its only accepted options are
`--width` and `--height`, which resolve the NativeUi title rectangles at the requested viewport.
The window smoke passes `--inspect-title --width 960 --height 720` separately from its overhead
capture command. Layout inspection rejects `--view` because camera orientation does not affect UI geometry.
The output includes all nine authored rows. Continue is hidden when no resumable game exists, so the
smoke uses entry coordinates only after creating a current save, when Continue is visible.
PGN `--review <file.pgn>` is not implemented. In the window, Review on the result panel opens the native review
screen. Review games on the title lists finished current/manual saves. A saved
result or a terminal replayed position distinguishes finished games; there is no
separate finished flag or automatic archive beyond current and three manual slots.
Two-player records do not identify a learner, so review selects the initial side
to move. Computer records retain the human side.

The complete scrollable SAN list and Start/Back/Next/End replay controls remain
available while neutral analysis runs. Each retained tick submits at most one
one-ply `Review::advance` job to an engine CPU work queue. WorkReady returns the
partial report; input callbacks do not search or wait for workers. Leaving pauses
before the next ply. An in-flight ply completes atomically, as required by the
review core, and its report is retained for re-entry to the same open game.
Replay uses a separate overhead framing beside the panel and restores the play
camera on exit. Unclear rows explicitly say that analysis cannot judge the move.

Select one of up to three teachable moments to read its label and learner sentence
from `teaching::explain`, then choose What changed?, Show the reply or Try again.
The explanation names the recorded move, searched alternative and legal reply;
it does not infer a tactical motif from a score difference. Show the reply steps
through the verified continuation on retained ticks and returns to the selected
recorded position. Try again accepts the searched better move and lets other
legal attempts retry. Return to line or leaving restores the recorded line.
Neither replay nor practice autosaves or rewrites the result. Regression tests
compare the entire application-store files byte for byte after those operations.

Review feedback uses the existing `teaching::explain::explain_move` and
`explain_reply` analysis, including its fact precedence and pieces-and-pawns prose.
Explanations and related lessons are computed on the existing CPU review worker
and cached with each retained moment, so reading or selecting a row does not run
search on the UI thread. Each newly retained moment adds two explanation budgets
of at most 100,000 nodes, separate from the review searches. Unclear or uncaused feedback keeps the wording
"searched continuation, not a proof of a tactical cause."

Typed motif tags in the course select the related lesson. The review shows its
title and ID. Open related lesson starts it in the existing Lessons player.
A locked lesson stays in review and names each unfinished prerequisite by title
and ID. Exit, Escape and lesson completion return to the same review cursor,
selected moment, analysis report and camera, with the recorded result and
computer configuration preserved. Analysis pauses during the lesson and resumes
on return; an in-flight ply can finish. Lesson progress persists independently,
without rewriting the reviewed game or its saved slots. Leaving practice to open
a lesson returns to the recorded line. The control is disabled when the selected
moment has no tagged lesson.
All classified motifs have authored course tags. Promotion links to first moves;
discovered attacks and checks link to the mixed-practice explanation. A course
without a matching tag produces no guessed lesson link.

Annotations are retained in memory for the same open game;
they are not persisted as part of the version-1 saved-game format. Reply playback uses
monotonic deadlines on retained ticks, not a scheduled timer service.

Analysis version 1 uses balanced evaluation, no personality, noise, policy queen
filter or wall-clock deadline. Each fresh search has **240,000 nodes**, twice the
largest ladder allowance (Soren: 120,000), a 16,384-slot transposition table, one
best candidate, and no normal depth cap beyond search's built-in 64. A larger
allowance is not a measured playing-strength guarantee. The best-root search
runs first. A separate search restricted to the played root receives the same
node ceiling and is capped at the best search's completed depth. Thus scores
compare the same position and horizon, not two consecutive positions at different
depths. If the played move is already best, its completed analysis is reused.
There are at most 480,000 visited nodes per ply; rules generation/evaluation costs
are outside node accounting. Tests override the allowance and depth.

Every row retains the position/history before play, played move/SAN, best move,
best PV in legal moves and SAN, both completed depths and actual node counts,
before/after evaluation, typed loss and any uncertainty reason. Centipawns are
relative to the mover for **both** evaluations. Mate winners remain absolute;
the after distance starts after the played move. Mate scores are never converted
to centipawns. Depth-zero fallbacks are not published as best moves or PVs.
Available draw claims follow search's take-the-claim policy; their absent search
continuation produces an unclear row, not a move recommendation.

Labels require both searches to complete at least **2 normal plies** and the same
depth. The searched best move is `best`, including a forced move. Other moves use
nonnegative best-minus-played centipawn loss with nominal thresholds: `good`
below 50, `inaccuracy` from 50, `mistake` from 150, `blunder` from 300. A different
move within **20 cp** of best is `unclear`; so is a loss within **10 cp inclusive**
of 50, 150 or 300. The resulting confident intervals are good 21-39, inaccuracy
61-139, mistake 161-289 and blunder at least 311 cp. Depth mismatch, insufficient
depth, no searched continuation, or a played-root score exceeding best also
produce `unclear`. These are search estimates, not proofs of move quality.

Losing a searched forced win is `missed mate`. Newly allowing the opponent a
searched forced mate is `allowed mate` (takes priority if both apply). Changing
only mate distance, or improving from a losing mate, is unclear unless the move
is the searched best. Every published line is replayed through the rules core;
mating score lines must end in checkmate with the stated winner and distance.
The refutation starts after the played move, showing the opponent's best reply
and continuation even if the actual opponent missed it. No tactical cause is
inferred from an evaluation difference alone.

`advance(&AtomicBool)` commits at most one ply and returns Advanced, Cancelled or
Complete. Cancellation is checked before each ply and can be cleared to resume;
a ply already running completes atomically. `progress()` reports completed/total
plies, and `report()` exposes the partial report. Search tables are fresh per
query, so pause/resume does not change the result. The report records analysis
version, node allowance and optional depth limit for later annotation replacement.

At most **3** teachable moments are retained by default (`max_moments` can change
or disable this). Only the player's confident mistakes, blunders and mate errors
qualify. Allowed mate ranks before missed mate, then centipawn losses descending;
ties use the earliest ply. Each contains the exact pre-move state/history, played
and better moves, and a rules-verified refutation for later What changed?, Show
the reply and Try again controls. An unclear result never becomes a teachable
moment. Original game moves and results remain separate from these alternatives.

Review unit tests use at most 5,000 nodes and depth 2. Debug CLI tests cover empty
records and rejection before side effects. The production-budget CLI regression
is compiled only in release mode and ignored by default; run it explicitly with
`cargo test --release --locked -j4 -p chess3d --test cli review_moves_is_cpu_only -- --ignored`.

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
runs each of six cases in a separate `xvfb-run` invocation, selecting a free display
starting at `:100`; it never uses the desktop. Requirements: Xvfb, xvfb-run, xauth,
xdotool, libX11 and Pillow. Missing prerequisites fail explicitly. The smoke
compares real window pixels to fresh offscreen frames, exercises
orbit/zoom/reset/resize/unmap/remap, checks saved positions, title-to-friend Enter
navigation, relaunch/Continue with exact restored pixels, Escape menu/resume and
clean window-manager exits. Every case uses a temporary `--data-dir`.

Window waits fail immediately if their current chess process exits, including
after relaunch, or after 180 seconds without satisfying the awaited condition.
Offscreen subprocesses allow the Rust capture watchdog's 1,200-second total bound.
Each case has a separate 1,200-second outer bound, allowing sequential waits and
pipeline compilation on a host running 4-6 times slower without the former
150-second cutoff. Six cases have a combined execution wait budget of 7,200
seconds, plus at most 60 seconds for process-group cleanup. These bounds do not
interrupt synchronous Python work outside subprocess waits. Per-case logs retain
flushed start/wait/ready/end progress and Xvfb diagnostics; failures report their
tails, and completed results are preserved. Pixel tolerances are unchanged.

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
