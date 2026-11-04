# Build an offline chess teacher on the existing engine

Build a playable local two-player game first, then add opponents and a short interactive course. Use Real Disunity's retained scene host, existing camera queries, native UI layout and current renderer. The main engine work is connecting UI to retained 3D presentation, implementing native interaction and widgets, and supplying shared persistence and background-work services. Mouse events, camera rays, text rasterization and retained object edits already exist in the inspected engine.

Use an original Rust alpha-beta chess engine in `rd-chess` for the first opponents and teaching analysis. This keeps packaging small, works offline, and lets explanations refer to inspectable chess features. Its strength and human-like behavior must be measured. Do not advertise Stockfish-quality analysis or externally calibrated ratings before those claims have evidence.

This design targets desktop players learning the rules through intermediate club concepts. The application is free, offline and usable without an account. Local play, lessons, puzzles and review have no daily allowance. Online matchmaking, cloud services, user chat, video courses and voice narration are outside the initial scope.

## 1. Inspection boundary and current state

Design date: 2026-11-04. This is a source survey and researched plan, not a report of a working implementation. No build, Cargo command, window test, commit, push or Hubnot mutation was performed for this note.

The inspected `real-disunity` checkout was clean at `ffc4c2ceeacd343c5e94cc983851e4a0aee399b2`. `rd-chess/Cargo.toml` pins the older engine revision `1ead975c7cf3efe672aad3e4f7a8050233843ad6`. Therefore an API described below as existing in the engine is not a claim that it exists at the game's current pin. Update the consumer pin deliberately after engine proposals are accepted and verified. Follow the current facade directly; do not introduce a compatibility layer for the old revision.

### What rd-chess owns today

- `src/lib.rs`: `Position`, `Piece`, `PieceKind`, `Side`, `Square`, document validation, bounded JSON loading and `Position::save_atomic`. Positions can be empty or lack kings. They are art/editor documents, not legal chess states.
- `src/scene.rs`: `render_scene`, `piece_lineup_scene` and scene assembly. Piece objects have transforms and shared mesh/material handles. The game does not yet retain the mapping from piece IDs to renderer IDs. A knight has a separate eye object that must move and disappear with its body.
- `src/geometry.rs`, `src/sculpt.rs`, `src/sculpt_surface.rs`, `src/textures.rs`: original procedural art. The board spans X/Y from -4000 to 4000 millimeters; its playing surface is Z=50 millimeters. Square centers are `(file * 1000 - 3500, rank * 1000 - 3500, 50)`.
- `src/presentation.rs`: `CameraPreset::{Hero, Overhead, Lineup}`. The overhead preset is deliberately tilted to avoid the orbit camera's pole.
- `src/main.rs`: CLI import/export/capture and `open_window`, which calls `worldrunner::scene::viewer::run`. Saving occurs before the viewer starts. Left drag currently orbits; it cannot move a piece.
- `src/tests.rs`, `src/regressions.rs`, `src/sculpt_tests.rs` and `tests/window_smoke.py`: useful art, serialization and real-window regression foundations. They do not establish legal play.

The package currently declares `license = "Proprietary"`, not MIT. The engine is MIT. This proposal preserves that separation and keeps the chess implementation eligible for a later MIT release without deciding the game's publication license.

### Concrete engine inventory

All paths in this table are relative to `real-disunity/`.

| Area | Existing implementation | Consequence for chess |
| --- | --- | --- |
| Window and mouse/keyboard input | `crates/scene-viewer/src/retained.rs`: `RetainedHost`, `RetainedDriver`, `RetainedInput`, `RetainedTick`. `retained/input.rs::window_input` translates pointer position, button press/release, wheel units, logical/physical keys, focus and resize. It releases held buttons and keys on focus loss. | Replace the static viewer call with a native retained driver. No new window loop or mouse backend is needed. Retained input lacks committed text/IME and an explicit display-scale event. |
| Event timing and interaction | `retained/session.rs` delivers events even during presentation pauses; scene edits occur in `update`. `RetainedEventHost` exposes exit and cursor operations. `world-simulation/src/input.rs` has timestamped `InputEvent`, `InputAction::{Key, Axis, Focus}` and fixed/dynamic `InputView`. | OS pointer input exists, but the native simulation/UI input contract does not contain pointer gestures, capture, focus navigation or widget activation. Implement those general semantics in the engine. |
| Camera and board-plane picking | `crates/camera/src/resolved.rs`: `ResolvedView::screen_ray`, `world_to_screen`, `screen_to_world`, `ScreenRay::intersect_plane`. `render-scene/src/view.rs::SceneView::resolved_view` exposes the same camera used for rasterization. `scene-viewer/src/camera.rs::OrbitCamera` supplies orbit/reset/zoom. | Square picking can already use a ray/plane intersection. Camera queries use top-left pixels and meters. Chess converts the board plane to Z=0.05 meters and owns the mapping to files/ranks. |
| Object picking | `world-simulation/src/physics2d/queries.rs::World::raycast` returns 2D `RayHit`s. The surveyed public APIs contain no retained-scene triangle/object picking service. | A 2D physics raycast cannot select a sculpted piece in the hero view. Add CPU object picking to the scene owners when enabling silhouette selection; do not build 3D physics for a chessboard. |
| Retained edits | `scene-viewer/src/retained.rs`: `ObjectId`, `SceneObjectEdit`, `RetainedUpdate::{Objects, Add, Remove, Replace, Resources, Camera}`. `retained/objects.rs` validates batches before mutation. `render-scene/src/draw/retained.rs` owns GPU-side retained behavior. | Moving, capturing, castling and changing the promotion mesh require game integration, not a new retained-state subsystem. Use one object batch per logical change and reuse resident meshes/textures. |
| Preparation and presentation | `scene-viewer/src/retained/readiness.rs` has bounded preparation, accepted/ready generations and background CPU preparation threads; publication remains in the renderer. `RetainedHost::apply` can return `ViewerError::BackPressure`. | Queue a pending visual transaction and admit it when capacity is available. Do not retry arbitrary errors, block the event thread or regenerate marble textures on each move. Picking must distinguish accepted edits from the objects actually shown. |
| Native UI layout | `world-simulation/src/ui/mod.rs`: `Canvas`, `CanvasMode`, `CanvasScale`, `RectTransform`. `ui/layout.rs`: `UiLayout`, `RectLayout`, screen corners and layout transforms. `CanvasGroup` and `CanvasRenderer` carry alpha, interaction flags and graphical content. | Extend this owner for hit testing, focus, buttons, selection lists, scroll panels and dialogs. Existing rectangles and interaction flags do not constitute working widgets. |
| Fonts and text | `ui/fonts.rs::FontBook` registers bounded SFNT bytes through `ab_glyph`; `TextRun`, `TextLayout`, `GlyphAtlas`, `FaceBook` and related font modules exist. `ui/text.rs::TextRun::validate` accepts printable ASCII, spaces and line breaks, rejecting unsupported text/markup. | English prose and ASCII SAN can ship first. Unicode piece glyphs, translated lessons, composed text and full shaping are separate work. The module summary mentions character wrapping, while the current `TextRun` overflow contract describes word wrapping; test the actual behavior before relying on it. |
| UI graphics and rendering | `ui/graphics.rs::extract_graphics` and `append_graphics` prepare canvas quads; `render-plan/src/ui.rs::extract_ui` prepares text contributions. `render-scene/src/dynamic.rs::DynamicQuad`, `draw/dynamic.rs::SceneGpu::set_dynamic_contribution` and `simulation.rs::WorldSceneContribution` use the existing renderer. | Text and 2D rendering already exist. The missing connection is a supported screen-space UI contribution over the retained shadowed 3D scene, with clipping and correct composition order. |
| Proof of the retained/UI split | `crates/play/src/driver.rs::PlayDriver` presents 2D contributions through `LiveDriver`. `play/src/retained_driver.rs::PlayerRetainedDriver` presents 3D objects and explicitly reports that UI quads are not drawn on that path. `scene-viewer/src/renderer/live.rs` rejects the dynamic shadow host. | Do not solve this by launching a second live host, dropping chess shadows or painting a game-specific HUD. Complete the shared retained renderer path so other engine consumers benefit. |
| Timers and work | `world-simulation/src/clock.rs::Clock` and `coroutines.rs::{WaitCondition, CoroutineId}` provide rational simulation time and scheduled script work. The viewer has `RetainedTick`, `RenderClock` and `app/cadence.rs` scheduling. | Timers exist. Coroutines do not move CPU search off the frame thread, and the preparation worker is renderer-specific. A small cancellable host-work facility is missing; a general async runtime is unnecessary. |
| Audio | `ComponentDesc::AudioListener` and `World::active_audio_listeners` hold listener state. The engine's own `csharp/Disunity/AudioSource.cs` throws because there is no audio output. No audio backend dependency appeared in the surveyed Cargo manifests. | Sound playback is a real engine gap. Build optional device output in the host and typed playback state in the simulation owner. Chess supplies original sound assets and decides which move triggers them. |
| Save files and settings | `crates/save/src/lib.rs` has `encode_world`, `decode_world`, `save_world_transactional`, `load_world_recovering`, bounded reads and backup handling, all specialized to `WorldDocument`. `play/src/player.rs` chooses a persistent directory for managed games; engine-authored `csharp/Disunity/PlayerPrefs.cs` stores typed values in XML. | There is no equivalent general native save-slot/settings API for a chess session. Extend `save` around its existing transactional I/O. Do not force a chess game into a terrain/world document or copy persistence code into each game. |
| Consumer surface | `crates/worldrunner/src/lib.rs` re-exports `scene`, `scene::camera`, optional viewer, `simulation` and `ui`; `world` exposes save. | Add only re-exports/features needed to reach existing owners. `worldrunner` remains implementation-free. |

## 2. Market: the opportunity is integrated offline teaching

Prices below are public USD reference prices observed or indexed during this research, before tax. They are not a checkout quote. Chess.com says [prices depend on location and exclude local taxes](https://support.chess.com/en/articles/8701404-where-can-i-find-the-prices-for-the-different-levels-of-membership); app stores, older subscriptions and promotions can differ.

| Product and tier | Reference price | Learning access and limits |
| --- | --- | --- |
| Chess.com Basic | Free | Unlimited human games; three puzzles/day; the New to Chess lesson section plus one additional lesson/day; one full Game Review/day; 20+ bot personalities; one Coach game/month. |
| Chess.com Gold | $6.99/month or $49.99/year | Unlimited puzzles and lessons, all bots, no ads. Unlimited Game Review is excluded. |
| Chess.com Platinum | $10.99/month or $79.99/year | Gold plus unlimited Game Review/analysis. |
| Chess.com Diamond | $16.99/month or $119.99/year | Platinum plus unlimited coach explanations and Insights, advanced statistics and analysis settings. |
| Lichess | Free; optional patron donations | No feature paywall. Unlimited puzzles, basic lessons, practice, studies and analysis board. The current feature page lists deep server analysis at 40 games/day for both ordinary accounts and patrons. Computer play is free. |
| ChessKid Basic / Gold | Basic free; public USD listing shows Gold $14/month, $29/quarter or $59/year | Basic has three puzzles/day, bot levels 1-3 and beginner lessons; Gold unlocks all bot/lesson levels, puzzles, videos and training tools. Gold also advertises unlimited Game Review. |
| Aimchess Standard / Premium | Standard free; Premium $7.99/month or $57.99/year | Standard lists one 40-game report/month, a weekly plan, one daily training session and warm-up. Premium adds unlimited reports/training, analysis of the last 1000 online games, more analysis depth and statistics. It is a personalized training product, rather than a graded-opponent ladder. |

Chess.com entitlements come from its [July 2026 membership comparison](https://support.chess.com/en/articles/8562418-what-does-each-level-of-premium-membership-get-me). The search-indexed [membership listing](https://www.chess.com/membership?ref_id=77490532) showed annual-equivalent amounts of $4.17, $6.67 and $10/month, although the live fetch did not expose its interactive price cards. The [US App Store listing](https://apps.apple.com/us/app/chess-com-play-and-learn/id329218549) corroborates the monthly amounts and some annual amounts; the complete annual totals are also reported in this [October 2026 comparison](https://onepawn.io/compare/chess-site-prices), which is a competitor's secondary source. Treat those totals as reference figures with that limitation. A Friends & Family Diamond option also exists for up to six people, but its current price was not verified.

Lichess documents its [equal free/patron feature set](https://lichess.org/features). ChessKid documents its [Basic/Gold limits](https://support.chesskid.com/en/articles/8858285-what-s-the-difference-between-a-free-account-a-gold-membership), [Gold review feature](https://www.chesskid.com/learn/articles/explore-the-exclusive-benefits-of-chesskid-gold) and [USD price listing](https://www.chesskid.com/hi/membership?c=nav). That localized listing is a public reference; the main membership page did not expose checkout amounts to the fetch. [Aimchess's own pricing page](https://aimchess.com/#pricing) supplied its prices and allowances. It displays $4.85 as the annual monthly equivalent, although $57.99/12 is about $4.83; use the annual total when comparing costs.

### Complaints worth designing against

Public discussions provide qualitative evidence, not a representative satisfaction survey. In [Chess.com Game Reviews = Terrible](https://www.chess.com/forum/view/general/game-reviews-terrible), players object to labels they cannot reconcile with a successful trap and to paying more for explanations. A successful trap can still be objectively unsound, so the design lesson is to show the opponent's refutation, not to label every win a good move. A separate [coach feedback thread](https://www.chess.com/forum/view/site-feedback/bot-coach-usually-excellent-being-wrong) illustrates the same need to distinguish a claimed sacrifice from a verifiable continuation.

Players also report bots alternating strong moves with implausible mistakes. The [Lichess offline-bot discussion](https://lichess.org/forum/lichess-feedback/will-lichess-add-more-bots-to-play-offline) describes a bot ignoring an attacked bishop, and a [bot-strength discussion](https://lichess.org/forum/general-chess-discussion/what-elo-levels-of-lichess-bots-under-play-with-computer) questions advertised ratings. These anecdotes motivate testing coherent difficulty and disclosing rating uncertainty. They do not establish how the current commercial engines are implemented.

Free access alone offers little differentiation from Lichess. Lichess also has offline mobile capabilities and anonymous play, so rd-chess must not claim that either feature is unique. The useful combination is an installed, no-account course that connects an understandable opponent, the player's own mistakes, a concrete explanation and immediate practice without leaving the same board. Bundle the starting course and puzzle pack; a clean installation with networking disabled must still teach and review a game. Make local progress exportable, omit streak pressure and purchases, and explain a few relevant mistakes instead of flooding beginners with evaluations.

## 3. Player experience

### Screens and navigation

| Screen | Behavior |
| --- | --- |
| Title | Continue saved game, New game, Learn, Puzzles, Review games, Settings and Quit. Show the marble board without making it the only navigation control. |
| New game | Choose local two-player or a named opponent, side, untimed/training options and assistance. Describe the opponent's habits in ordinary language. All levels are selectable; course progress never locks play. |
| Play | Board plus side-to-move/check/result status, SAN move list, opponent, Undo, Hint, Restart and Menu. Promotion is a four-choice modal. Draw claims, offer/accept draw and resignation are explicit actions. |
| Lesson browser | Recommended next lesson, searchable topic groups, prerequisites and saved progress. Prerequisites guide the player; allow browsing ahead. |
| Lesson player | One instruction beside the board, a concrete task, progressive hints, retry and back/next. The player manipulates real pieces and can replay the explanatory line. |
| Puzzle trainer | Choose motif or mixed practice, difficulty and optional review of previous mistakes. After solving, play out the consequence and explain the motif. |
| Post-game review | Replay controls and a short list of teachable moments. Each offers What changed?, Show the reply, Try again and a related lesson. Keep the complete move list accessible. |
| Settings | UI scale, board contrast, orientation, reduced motion, input bindings, sound level/mute and local data management. Engine widgets implement these controls; chess supplies labels and values. |

One game-owned `Screen` enum and typed transitions are sufficient. Avoid a generic screen framework in the game. Shared focus, modal behavior, scrolling, layout and widget state belong to engine UI.

### Input and camera

Left click selects a side-to-move piece and displays legal destinations. Clicking another friendly piece changes selection. Clicking an illegal destination leaves the state unchanged and gives a short reason when useful. Highlight the selected square, legal destinations, last move and checked king with shapes as well as colors.

Left press followed by motion beyond a DPI-aware threshold starts dragging. Show a presentation-only piece preview; the rules position changes only after a legal release. Releasing outside the board, losing focus, pressing Escape or opening a modal cancels the drag. The synthetic button releases already generated on focus loss must never accidentally commit a move. Use engine capture/cancellation semantics rather than a chess-specific gesture recognizer.

Use right drag for orbit, wheel for zoom, Home to restore the chosen preset, and a visible Flip board action. Disable orbit during a piece drag. Escape first cancels a gesture/dialog, then opens the pause menu; quitting requires the menu or window close. Arrow keys move a square cursor and Enter/Space select or move, so dragging is never required.

Default to the current high overhead preset for readable play. Keep orientation fixed during local two-player games unless the user enables turn-based flipping. Hero view remains available for play after silhouette picking works. A true orthographic top-down view needs a retained-camera lens/up contract change even though `SceneView::orthographic_with_up` exists below the host; it is optional and does not block the initial release.

The 3D board adds recognizable material, orientation and visible movement. Teaching arrows and threat lines show relationships on the board, with labels kept in readable screen space. It must remain possible to disable movement animation and use high-contrast pieces. There is no chess-specific drawing pass, HTML window, egui renderer or separate 2D board renderer.

Undo in local play removes one ply. Against a computer it cancels outstanding analysis and returns to the previous player decision, removing the computer reply when present. Hints progress from a question, to the relevant piece/square, to a candidate move, to a line. Assistance is recorded locally for review but never penalizes access.

## 4. Ownership and contracts

### Game modules

Keep the following as modules of the existing `chess3d` package initially:

- `rules`: position state, legal move generation, attacks, outcomes, repetition, make/unmake and FEN/SAN/PGN.
- `game`: turns, history, assistance, opponent selection, result and screen transitions.
- `opponent`: search, evaluation features, candidate selection and opponent profiles.
- `teaching`: curriculum validation, lesson execution, motif detection and grounded explanation templates.
- `presentation`: existing art/camera choices, piece-to-object identities and applying domain changes to retained objects.

No generic UI library, file store, task executor, audio mixer or picking implementation belongs here. Ordinary chess-specific event handling is game logic; general event routing and widgets are engine behavior. Keep one engine `World` for native UI and any engine simulation state. The chess position is a rules value, not another engine world or entity registry.

### Rules authority

The existing `Position` remains an explicitly named scene/editor document. Do not infer side to move, castling rights or history from its piece placement. A playable game starts from the standard initial state or an explicitly validated FEN/game document. Importing an editor layout into a game requires supplying the missing state and passing playable-position validation; impossible positions remain usable for art inspection only.

Proposed Rust contracts below describe new game types, not APIs already implemented:

```rust
pub struct ChessState {
    // Private bitboards, side, castling rights, en-passant state and move counters.
}

pub struct Game {
    // Owns the current ChessState, history and repetition information.
}

pub enum MoveRequest {
    Move { from: Square, to: Square },
    Promote { from: Square, to: Square, into: PromotionPiece },
}

pub enum MoveOutcome {
    Applied(MoveDelta),
    PromotionRequired { from: Square, to: Square },
}

impl Game {
    pub fn legal_moves(&self) -> Vec<LegalMove>;
    pub fn play(&mut self, request: MoveRequest) -> Result<MoveOutcome, MoveError>;
    pub fn undo(&mut self) -> Result<MoveDelta, HistoryError>;
    pub fn status(&self) -> GameStatus;
}
```

`PromotionPiece` permits queen, rook, bishop and knight only. `LegalMove` cannot be freely constructed by consumers; revalidate its applicability if exposed across asynchronous boundaries. `MoveDelta` includes the mover, captured piece and its square, optional rook movement, and promotion. It carries enough identity to undo a capture and update both knight meshes. Selection, animation and analysis never modify the rules state independently.

Implement castling through attacked squares, en passant exposing check, underpromotion, checkmate and stalemate explicitly. Threefold repetition and the fifty-move rule are claimable; fivefold repetition and seventy-five moves are automatic, with checkmate precedence. Repetition identity includes side, castling rights and en-passant availability when it changes legal moves. FEN alone cannot restore repetition history, so a saved game includes its initial state and move history.

A full FIDE dead-position decision covers more than the usual insufficient-material cases, including some blocked positions. The first playable milestone must name its supported automatic dead-position cases and retain agreed draws for other cases. Do not advertise complete FIDE adjudication until that broader behavior is implemented and tested. Timed competition and flag-fall adjudication are deferred; the first release is untimed.

### Scene publication and picking

Build meshes, textures and materials once. Store `PieceId -> Vec<ObjectId>` in the game's scene assembly result, including knight eyes. Castling replaces two piece placements atomically; capture removes all objects of the victim; promotion replaces mesh handles and any auxiliary objects. Undo restores the same identities. Chess owns these mappings because they encode its art.

For the first fixed-overhead board, use the existing `screen_ray` and `intersect_plane` API. In meters, accept `-4 <= x < 4` and `-4 <= y < 4`, then compute `file=floor(x+4)`, `rank=floor(y+4)`. Keep the top-left pixel convention used by `ResolvedView`; native UI uses a lower-left screen basis, so the engine input/UI boundary performs that conversion exactly once.

A ray through a tall piece can meet the board behind it. When hero-view selection is enabled, query the nearest pickable mesh first, map its ID to a chess piece, and use the plane only for empty destinations. Captured pieces and ornamental rails must not be selectable. Match picking to the published camera and object generation, not a mixture of current CPU objects and an older displayed frame. Freeze board interaction during a pending move publication; menus and camera controls remain responsive.

### Worker and storage boundaries

A search gets an immutable position snapshot, draw-history context, request ID, session generation, limits and deterministic seed. It returns typed analysis or cancellation. The main thread alone applies a move after checking request identity and legality. Undo, restart, lesson changes and leaving play cancel the request and increment the session generation, including when the same position occurs again.

Engine persistence takes explicit paths, bounded payload bytes and slot identifiers. Chess serializes the session schema, initial position, moves, piece identities, opponent configuration, assistance and lesson progress. Save committed logical states only; exclude GPU handles, worker handles and half-completed drags. Unknown versions and corrupt files produce visible errors. Recovery from a previous save is a reported action rather than a silent replacement.

## 5. Opponents: original search with coherent weaknesses

### Alternatives and licensing

| Approach | Benefits | Costs and license consequences | Decision |
| --- | --- | --- | --- |
| Original Rust alpha-beta engine | Reuses one legal-move core; compact CPU-only deployment; evaluation features support explanations; no model download. | Search quality, calibration and human-like errors require substantial testing. A new engine will be weaker than Stockfish and may misjudge quiet positions. | Use for the initial release. Limit teaching claims to validated motifs and analyzed lines. |
| Stockfish linked into the application | Strong analysis, mature search and strength controls. | [Stockfish is GPLv3](https://stockfishchess.org/about/). MIT code is GPL-compatible, but a distributed combined program cannot be offered solely under MIT or the current proprietary terms while incorporating GPL-covered code. Private internal use does not require public source release. Shipping to testers/customers is distribution even if the repository stays private. | Do not link it into Real Disunity or the initial game. |
| Stockfish as a separate UCI executable | Strong independent analysis without embedding its implementation into the engine. | A genuinely separate process speaking standard UCI can be an aggregate, with Stockfish's notices, exact corresponding source and distribution obligations preserved. Process separation alone does not settle derivative-work questions; see the [GNU GPL FAQ on aggregation](https://www.gnu.org/licenses/gpl-faq.html#MereAggregation). Adds packaging, process supervision and license work. | Suitable as an external development reference, not a first-release dependency. Reconsider only if stronger review becomes a product requirement. |
| Maia-style human move model | Skill-conditioned move distributions address human tendencies directly instead of relying on search depth alone. | [Maia-2](https://github.com/CSSLab/maia2) declares MIT; its supported implementation uses Python/PyTorch. Published checkpoint mirrors also [identify MIT](https://huggingface.co/shermansiu/maia2/blame/main/README.md), but verify the exact upstream artifact and full dependency licenses before bundling. Original Maia/Lc0 is GPL, while [Maia-3 code](https://github.com/CSSLab/maia3) and the [79M checkpoint](https://huggingface.co/UofTCSSLab/Maia3-79M) identify AGPLv3. Some other model cards are ambiguous, so the family name is insufficient license evidence. | Defer. Native inference adds a runtime and model packaging, and a human-move predictor still needs tactical validation for teaching. |

GPL distribution obligations depend on whether the delivered software forms a combined work. Real Disunity's independently licensed MIT source can remain MIT. Avoid copying GPL engine code into either repository. This assessment identifies engineering constraints; a future distribution needs its own license review.

### Search and move selection

Start with bitboards, precomputed leaper attacks and straightforward sliding rays. Use one tested move generator for play, puzzles and search. Add negamax alpha-beta, iterative deepening, quiescence, move ordering and a bounded transposition table incrementally. Evaluation initially covers material, piece-square placement, mobility, king safety, pawn structure and passed pawns. Avoid speculative pruning until correctness and measured search cost justify it.

Separate analysis from opponent policy. The analysis configuration searches without personality or noise. Opponent policy limits its candidate set, search budget and evaluation attention, then samples among plausible candidates using a saved seed. A beginner can overvalue an attack or miss a discovered threat consistently; it should not play a long forcing combination and then intentionally discard its queen because a random-blunder counter fired.

Candidate generation should reflect the level's learned motifs, captures, checks, recaptures, development and existing plan. All candidates remain legally generated, including every legal check evasion. Higher levels examine a wider set of opponent replies. A persistent preference for development, exchanges or initiative influences close choices across several moves, without forcing a losing move to maintain a personality. Use bounded evaluation perturbation within that policy, never uniform sampling from all legal moves.

Do not secretly weaken an opponent when the player is losing. Offer an explicit easier opponent or a retry from the last decision. An optional minimum thinking display uses host timers; it never sleeps the frame thread or pretends to be measured computation.

### Eight-level ladder

These are design targets roughly expressed in online rapid terms, not measured Elo and not equivalent to FIDE ratings. Show level names and behavioral descriptions first. Publish numbers only after calibration against a named pool and time control.

| Level | Name and target range | Intended style and strength control |
| --- | --- | --- |
| 1 | Pip, first games, roughly 300-500 | Favors direct captures and simple development; notices immediate checks and obvious recaptures but misses indirect threats. Small candidate and node budgets, minimal positional evaluation. |
| 2 | Rowan, roughly 500-700 | Tries to protect pieces and castle; often overlooks forks involving a quiet setup. Adds hanging-piece awareness and a short threat scan. |
| 3 | Mira, roughly 700-900 | Attacks actively and overvalues initiative; knows one-move forks and pins. More replies examined, limited pawn-structure judgment. |
| 4 | Ellis, roughly 900-1100 | Develops steadily and seeks favorable exchanges; misses some intermediate moves. Broader candidates, stable basic king safety and material accounting. |
| 5 | Nia, roughly 1100-1300 | Prefers open lines and tactical play; can overextend pawns. Larger search budget, common two-stage tactics and recapture sequences. |
| 6 | Tomas, roughly 1300-1500 | Patient, values pawn structure and endgames; sometimes allows initiative for long-term gains. Full initial evaluator and broader defensive search. |
| 7 | Imani, roughly 1500-1700 | Balances prophylaxis and activity; chooses practical complications between close alternatives. Small bounded score perturbation, deeper completed iterations. |
| 8 | Soren, roughly 1700-1900 | Uses the strongest validated configuration, with variation only among close moves. No scheduled mistakes; this range is withheld if the engine cannot achieve it. |

Use deterministic node budgets for strength comparisons and a separate elapsed-time safety cap for responsiveness. Do not equate depth with rating. Evaluate a fixed position suite plus paired games with reversed colors and shared starting positions, measure illegal moves, missed immediate recaptures, repeated piece giveaways, draw behavior and score ordering, then conduct blind human play sessions for perceived coherence. A higher level must beat lower levels reliably over a stated sample; adjacent levels need not win every game. Seeded reproduction must permit investigation of a specific implausible move.

### Teaching from the same analysis

A proposed `AnalysisRequest` contains position/history, generation, limits and purpose. `Analysis` contains completed depth, nodes, score perspective, candidate lines and typed facts. Represent scores as `Centipawns(i32)` or `Mate { winner, plies }`; never subtract mate scores as ordinary centipawns. Compare the chosen move and alternatives from the same player's perspective and comparable search budgets.

Generate text from verified facts: an undefended piece, a pin to the king, two attacked targets, a forced recapture, loss of castling rights, a passed pawn or a demonstrated mating line. Templates must name the actual squares and show the continuation. For example: "Moving the knight leaves the rook undefended. Black can take it with ...Bxa1. Try a move that keeps the rook defended." Render that only when the line and defense relationship have been checked in the current position.

An evaluation difference alone cannot establish a cause. If no motif explains the result, show "This line scores better in the current search" with the line and analysis limits. If a tactic is unresolved, say so and omit a confident blunder label. Avoid an LLM dependency and fabricated explanations. Human-authored lesson prose remains the authority for broader strategic concepts; analysis verifies the exercise and offers counterplay.

## 6. Teaching system

### What established teaching suggests

The [Dutch Steps Method](https://www.stappenmethode.nl/en/) organizes instruction and practice into six steps. [Step 1](https://www.stappenmethode.nl/en/step1.php) emphasizes movement, attack and defense before mating; [Step 2](https://www.stappenmethode.nl/en/step2.php) combines tactics, activity, opening principles and basic endings. This supports short tasks with repeated practice and analysis of the learner's games. It does not support rushing straight from piece movement into memorized openings.

[Seirawan's Winning Chess introduction](https://everymanchess.com/products/play-winning-chess) combines rules, notation, tactics and questions. [Polgar's 5334 Problems](https://www.hachette.com.au/bruce-pandolfini-laszlo-polgar/chess-5334-problems-combinations-and-games) illustrates repeated pattern practice with mates and combinations. [Chernev's Logical Chess](https://www.newinchess.com/logical-chess-move-by-move-7815) supplies the model of explaining decisions throughout a complete game. Adopt those teaching mechanisms with original writing and examples.

[Silman's Complete Endgame Course](https://www.silmanjamespress.com/shop/chess/silmans-complete-endgame-course/) sequences endgame knowledge by playing level. [How to Reassess Your Chess](https://books.google.com/books/about/How_to_Reassess_Your_Chess.html?id=jFehQQAACAAJ) organizes positional understanding through imbalances and explicitly targets more experienced players. [Nimzowitsch's My System and Chess Praxis](https://www.newinchess.com/my-system-chess-praxis) inform later themes of blockade, prophylaxis and pawn structure. These are references for curriculum design, not material to copy or mandatory reading for beginners.

[Yusupov's course](https://www.simonandschuster.com/books/Build-Up-Your-Chess-1/Artur-Yusupov/9781906552671) interleaves tactics, strategy, calculation, openings and endings, testing understanding after instruction. [Pandolfini's Ultimate Guide](https://www.simonandschuster.net/books/Pandolfinis-Ultimate-Guide-to-Chess/Bruce-Pandolfini/9780743260985) uses a student-teacher dialogue from fundamentals through planning. Translate that into questions the learner answers on the board and short explanations of the answer.

### Progression and mastery checks

The browser presents the following progression. Basic endings and tactical safety recur throughout, rather than being withheld until the final unit.

| Stage | Content | Evidence of understanding |
| --- | --- | --- |
| 1. Rules and board vision | Coordinates, movement, capture, attack/defense, check evasions, legal king moves, castling, en passant, promotion, stalemate and notation. | Move a specified piece, identify attacked squares, escape check and distinguish legal from illegal castling in unseen positions. |
| 2. Finish a won position | Mate versus check, escape squares, ladder mate, king and queen versus king, king and rook versus king; avoid stalemate. | Deliver mate against legal resistance from several placements and explain which escape squares were removed. |
| 3. Tactical patterns | Hanging pieces, forks/double attacks, absolute and relative pins, skewers, discovered attack/check, double check, removal of defender, deflection, attraction, interference, clearance, zwischenzug and back-rank motifs. | Solve both attacking and defensive tasks, with mixed unlabelled positions after motif practice. |
| 4. Openings with reasons | Center, development, king safety, tempo, pawn breaks and checking the opponent's threat. Introduce the Italian, Queen's Gambit, French, Caro-Kann and Sicilian as families of plans. | Choose a sensible developing move, identify the intended pawn break and explain a tempting early queen move's cost. |
| 5. Middlegame plans | Pawn chains, isolated/doubled/backward pawns, open files, weak squares, outposts, good/bad bishops, space, exchanges, prophylaxis and improving the least active piece. | Name a target, choose a candidate plan, test the opponent's reply and play several moves toward it. |
| 6. Endgames in increasing depth | King activity, pawn races, opposition, pawn-square rule, promotion tactics, rook activity, Lucena/Philidor, minor-piece endings and converting an extra pawn. | Play out a win or hold a draw from multiple starting positions; distinguish a theoretical result from an analysis estimate. |

For the Italian, explain that `Bc4` develops toward the center and f7, while `c3` and `d4` can challenge the center when tactics permit. For the Queen's Gambit, explain why `c4` pressures d5 and how development supports recovering or compensating for a pawn. For the French and Caro-Kann, connect their central support to pawn breaks and piece activity. Introduce the Sicilian only after the learner understands asymmetry and competing plans. Each named opening needs an opponent deviation and a reasoned response, rather than a single memorized sequence.

### File-authored lessons

Store original lessons as JSON under proposed `assets/lessons/`, using the existing serde/serde_json dependencies. A lesson specifies an ID, schema version, title, prerequisites, concept tags, starting FEN, typed steps, accepted goals, hints, feedback and a brief source/license field. Load and validate the entire small starting course before enabling it; malformed content reports its file and step rather than failing mid-lesson.

Step variants are `Explain`, `ChooseSquare`, `PlayMove`, `PredictReply`, `ChoosePlan` and `PlayToGoal`. Branches may depend on typed answers or known chess predicates. Do not embed arbitrary scripts or add a lesson-specific scripting language. The game contains the chess lesson interpreter; the engine contains only its general text, widgets and scene interaction.

Illustrative original lesson data:

```json
{
  "version": 1,
  "id": "mates.close-the-escape",
  "title": "Close the escape squares",
  "requires": ["rules.check"],
  "tags": ["mate_in_one", "king_support"],
  "source": {"kind": "original", "author": "rd-chess"},
  "start_fen": "7k/8/5KQ1/8/8/8/8/8 w - - 0 1",
  "steps": [
    {
      "kind": "explain",
      "text": "Your king can support the queen. Look for a check that leaves no legal reply."
    },
    {
      "kind": "play_move",
      "goal": {"kind": "checkmate", "within_plies": 1},
      "hints": ["Which squares can the black king escape to?", "Try bringing the queen closer."]
    }
  ]
}
```

Validate every FEN and authored line through the same rules core. Predicate-based tasks accept any move satisfying the stated goal; do not reject an alternative mate because it differs from the author's preferred move. A specific-line exercise must say what it is practicing and handle equally valid alternatives explicitly. Undo/retry restores the lesson checkpoint, and hints advance monotonically until a new attempt.

Record success, hints used and recurring concepts locally. Recommend a related task after a demonstrated mistake, then revisit it in a later session with a different position. Use a small deterministic scheduler with an injected date for tests. Avoid opaque skill scores and claims that puzzle ratings directly measure the player's game strength.

### Puzzle sources and content rights

The [Lichess database page](https://database.lichess.org/#puzzles) explicitly releases database exports under CC0, allowing modification and redistribution, including commercial use. Its puzzle CSV supplies FEN, UCI moves, rating and themes. The FEN is before the opponent's setup move: apply the first move before presenting the puzzle, and start the player's solution at the second move. Test this import detail directly. Accept all mating moves in mate-in-one tasks, as the database notes.

Bundle a curated, bounded CC0 subset with puzzle IDs and a plain attribution notice, plus original exercises for elementary concepts the database does not teach well. Validate every selected solution; filtering and conversion happen before packaging, with no first-launch download. Do not assume the CC0 export license covers Lichess site prose, icons or code.

Teach ideas from books in new prose. Do not scan, paraphrase chapter-by-chapter, copy diagrams/problems or reproduce an author's selection and annotations. Modern translations of older works can remain copyrighted even when an original edition is public domain somewhere. Publicly accessible samples are references, not redistribution permission. Any additional source requires an explicit reusable license for the actual material.

### Review the player's own game

Analyze committed positions in the background with the neutral search configuration. Identify a few high-confidence tactical opportunities or errors, preserve the original result, and distinguish what happened from what best defense could have done. Let the player replay, try an alternative against analysis, return to the actual line and open a related lesson. Save annotations with the analysis version and limits so later analysis can replace them explicitly. Opening advice must refer to a threat or positional reason, not merely a move outside the small teaching repertoire.

## 7. Prioritized engine gaps

These are proposed issue scopes, not issues created during this task. Each belongs in existing engine crates and needs a synthetic public regression independent of private chess assets. Numbers express delivery priority. The first two block a complete mouse-playable board with visible status and promotion; the third completes comfortable selection in the hero view. Existing input, ray/plane math and object edits should be reused without replacement projects.

### G1. Compose native UI with retained 3D frames

**Owner:** `render-plan`, `render-scene`, `scene-viewer`.

**Scope:** Add a typed retained contribution for existing native UI text and graphics, reusing `SceneGpu` dynamic resources and the current frame submission. Draw screen-space UI after scene lighting/post-processing with explicit pixel projection, alpha blending and clipping; it must not inherit board depth, shadows, exposure or camera motion. Update retained window and capture paths together, preserve bounded resource lifetime, and make `play` consume the same facility instead of omitting its UI.

**Test first:** A synthetic shadowed cube with a label and translucent panel must show identical UI placement in window/capture, survive resize and resource replacement, and remain unchanged under camera orbit and exposure changes. Assert that text stays above geometry and that a clipped panel cannot paint outside its bounds. Repeated text updates must not re-upload the cube's resources.

### G2. Add native pointer routing, focus and basic controls

**Owner:** `world-simulation::input`, `world-simulation::ui`; `scene-viewer` supplies OS observations.

**Scope:** Extend the typed native input contract for pointer positions/buttons/wheel and explicit cancellation, then hit-test the existing resolved UI layout in drawing order. Add focus, pointer capture, disabled controls, modal blocking and a minimal button/radio-choice contract returning typed actions. Convert top-left host pixels to UI coordinates once, and ensure focus loss cancels gestures before any release can activate a control.

**Test first:** Press/release activates a button once, pressing outside and releasing inside does not, overlapping controls choose the visible top target, and modal choices consume input before the board. Test focus loss during press/drag, disabled controls, keyboard traversal, repeat events and display scaling. Exercise a real window's four-choice dialog and verify a click does not also move the object behind it.

### G3. Pick retained scene objects against a published frame

**Owner:** `render-plan` for CPU mesh/bounds intersections; `scene-viewer` for retained identities and published frame snapshots. Reuse `camera::ScreenRay`.

**Scope:** Add a CPU picking query over resident meshes with object filters and nearest-hit distance, plus a frame-coherent snapshot of the view and object transforms. Start with bounding-volume rejection and triangle intersection for small scenes; add acceleration only after measurement. Keep IDs stable through edits and make misses ordinary results; do not reuse 2D physics or read the GPU depth buffer to decide which object was clicked.

**Test first:** Near/far overlapping meshes, transformed objects, nearest-hit ties, filtered decorations, moved/removed objects and both camera handedness conventions. Hold publication behind an accepted edit and prove the query targets the displayed generation. In a real window, click the top of a tall object where the board-plane intersection falls behind it, then resize and repeat.

**Dependency note:** Fixed-overhead square clicking can precede this issue using existing plane queries. Hero-view silhouette selection must wait for it.

### G4. Add menu layout, scrolling and reusable selection widgets

**Owner:** `world-simulation::ui`, with extraction in `render-plan` and drawing in `render-scene`.

**Scope:** Extend existing rectangles and canvas graphics with stack/grid layout, bounded scroll views, selection lists, sliders, toggles and modal containers. Keep measurement, clipping and hit regions consistent, including text children whose current overflow handling is not a scroll-viewport clip. Add reusable widget styling and visible focus states; titles, menu structure and actions remain caller-authored data.

**Test first:** Keyboard and pointer operate the same selection, scroll offsets remain within bounds, clipped children cannot receive clicks, and nested modal focus returns to the opener. Resize a real window containing a long list and scaled text, then select the final item without overlapping controls. Do not require a full application screen framework.

### G5. Expose display scale and committed text to native UI

**Owner:** `scene-viewer` for platform events; `world-simulation::input` and `ui` for typed text editing and focus behavior.

**Scope:** Extend retained input with display-scale changes and committed text/IME lifecycle instead of deriving characters from physical key names. Add a bounded text-field model with cursor, selection and editing commands, while keeping document parsing in the consumer. Existing `WindowSizeUnit` remains the initial-size policy; it is insufficient by itself for a window moved between differently scaled displays.

**Test first:** Scale changes keep visual and hit-test geometry aligned; test key repeat, composed input, selection deletion and cancellation. In a real window, edit a local save label and move/resize the window across available scales. English/ASCII release content can precede broad script support, but unsupported text must be reported rather than silently lost.

### G6. Provide native settings and application save slots

**Owner:** `save`; platform path selection in `scene-viewer`; re-export through `worldrunner`.

**Scope:** Factor reusable bounded transactional byte I/O from the current `WorldDocument` save implementation and add typed slot identifiers, listing, metadata and settings values. Keep payload schemas and version interpretation with the caller, preserving the existing world-document API on the same I/O implementation rather than creating another store. Accept explicit directories and size limits, and report corrupt data, permission failures and recovery source without silently replacing user data.

**Test first:** Interrupted replacement preserves the previous file; oversize/unknown payloads fail before allocation or decode; slot names cannot escape the root; separate application roots cannot collide. Relaunch a real window after changing a setting and saving a synthetic session, and verify both restoration and a visible read-only-directory failure. Managed preferences should eventually delegate to the Rust service through existing boundaries, without importing CLR or filesystem access into `world-simulation`.

### G7. Run bounded cancellable CPU work outside the event thread

**Owner:** `scene-viewer` host services.

**Scope:** Add a small generic work queue with explicit capacity, cancellation token, typed completion and deterministic shutdown; it must accept work without referencing a game or renderer. Use standard threads/channels internally and a host wakeup for completions, including when presentation is paused. Keep CPU jobs separate from GPU preparation and make replacement/cancellation and full-queue outcomes explicit; do not introduce a general async runtime or expose renderer workers to games.

**Test first:** A synthetic job stalls until signaled while the window continues accepting input; cancellation and window close release work without stale callbacks or leaked threads. Test queue saturation, worker failure, completion during occlusion and shutdown with a queued result. The chess consumer separately tests generation rejection after undo/restart and validates returned moves.

### G8. Add basic native sound playback and device output

**Owner:** `world-simulation` for clip/playback state and commands; `scene-viewer` for device output and decoding/service wiring.

**Scope:** Introduce bounded short-clip playback, gain, mute, stop and completion using a maintained permissively licensed audio dependency selected during implementation. Start with nonspatial sound and an explicit output configuration; no new crate, second host or game-owned mixer is needed. Device absence or loss is an explicit service status shown by the application, and the user may choose sound disabled; do not claim success from a silent dummy backend.

**Test first:** Deterministic software mixing verifies gain, overlapping clips, clipping policy, stop and mute; malformed clips are rejected. A real-window control triggers an original tone, verified by device loopback or an audible manual check, and device-loss handling keeps the window responsive. Audio is optional for first play and first lessons; no meaning is conveyed by sound alone.

### G9. Add readable text flow and reusable inline formatting

**Owner:** `world-simulation::ui`; atlas and draw updates through existing `render-plan`/`render-scene` services.

**Scope:** Extend `TextRun` with structured styled spans, word-aware paragraph layout and clipping suitable for long instructional panels, while preserving font/glyph limits. Cache unchanged font-size/glyph work instead of rebuilding complete text resources for unrelated board updates. Add Unicode coverage/shaping only with explicit supported-script contracts and a licensed bundled font; ASCII SAN and English lessons remain the initial scope.

**Test first:** Line breaking, font metrics, links/focus, clipping and resource reuse under repeated updates; reject missing glyphs and unsupported shaping clearly. In a real window, read a multi-paragraph panel at several sizes and UI scales without lost lines or board overlap. Verify that a board move does not allocate another font atlas when the panel is unchanged.

### G10. Render shared annotations and complete 2D graphic geometry

**Owner:** `world-simulation::ui` for typed visual primitives; `render-plan` for extraction; `render-scene` for drawing.

**Scope:** Provide caller-authored lines, arrows, circles and labels in screen space or attached to world points, with explicit depth-tested versus always-visible behavior. Extend the existing graphic path to handle the required triangle geometry, since `UiGeometry::quads` currently counts unmatched triangles as omitted. Keep the primitive vocabulary game-neutral; chess decides which attack, square or line deserves emphasis.

**Test first:** Synthetic arrowheads survive extraction, world anchors follow orbit/resize, screen labels retain size, and depth/clip policies have distinct pixel results. Verify multiple annotations in a real window and confirm reduced-motion settings suppress animated emphasis. A basic square highlight can already be an ordinary scene object; do not block first play on this issue.

### G11. Expose native UI accessibility semantics

**Owner:** `world-simulation::ui` for roles, labels, value and focus state; `scene-viewer` for platform accessibility delivery.

**Scope:** Publish a semantic tree from the same widgets used for rendering and input, without putting platform calls in the simulation crate. Support buttons, lists, dialogs, text and status changes so consumers can describe a board through accessible controls. Keyboard operation belongs in G2/G4; this issue supplies assistive-technology access rather than promising that keyboard support alone provides it.

**Test first:** Semantic focus and visual focus agree, dialogs restrict navigation and updates preserve stable node identities. Use a platform screen reader with the real window to open a menu, choose an option and hear a changed status. Chess supplies square/piece names and move announcements, and must validate those separately.

### Work that is deliberately not an engine gap

Chess legality, opponents, FEN/SAN/PGN, opening names, puzzle selection, lesson prerequisites, motif recognition and move explanation remain in `rd-chess`. Moving a retained object, orbiting a camera, obtaining a screen ray, rasterizing ASCII text and scheduling simulation waits already have owners and implementations. Do not turn any of these into a replacement framework. Fully general 3D physics, skeletal animation and neural inference are unnecessary for the proposed first game.

## 8. Test-first delivery as small proposals

Every proposal begins with a failing behavior test in its owner, followed by implementation and a real-window acceptance step. Engine tests use synthetic public fixtures; the game uses its original art and authored/CC0 positions. Implementation must respect existing lint limits, keep the facade thin and never weaken checks. Claim bounded engine work and inspect current shared claims before implementation; this note makes no claim on another session's work.

Use the existing Linux `tests/window_smoke.py`/Xvfb pattern with actual OS pointer and keyboard injection. Preserve the orbit/reset/resize regressions while adding move scenarios. On macOS launch through the established AquaJob path, never Terminal automation. Each acceptance run captures a few meaningful frames and verifies game state as well as visible effects; a logged command or changed FEN alone cannot prove a visible move. The following are future checks, not checks run for this design.

| Proposal | Scope and tests written first | Real-window verification |
| --- | --- | --- |
| P1. Rules core | Add private playable state, legal moves, make/unmake and outcomes before any bot. Test initial-position perft 20/400/8902/197281 through depth four, then independently checked castling, pin, en-passant, promotion and mate/stalemate fixtures. Test undo restores counters, rights and repetition state; use randomized legal sequences as property tests. | Load a rules-generated end position through the existing scene path and exercise orbit/reset/resize. Compare displayed placement with the generated position. This proves art integration only; explicitly label it noninteractive. |
| P2. Adopt current retained engine surface | Update the engine pin and replace the static viewer call with `RetainedDriver`; retain piece/object mappings and batch move deltas. Tests assert castling, capture, knight eyes, promotion and undo edit sets without rebuilding assets. | Use temporary test-driving commands through the real window to apply a short legal sequence, capture the result, undo it and verify restored pixels/identities. Keep the commands game test controls, not a second host. |
| P3. Retained UI composition | Deliver G1 as an engine proposal using existing text/graphics extraction. Tests first cover shadowed retained geometry plus screen-space text and capture parity. | Show a synthetic retained 3D scene with a status panel, orbit and resize it, and capture both window and engine output. Then validate the consumer pin before adopting it. |
| P4. Native interaction and two-player board | Deliver G2, then a small game proposal for square clicking, legal destinations, turn/check/result display, four-way promotion and undo. Test input cancellation and illegal-move immutability before wiring clicks. | Click e2-e4, e7-e5 and a capture; reject an illegal move; play short fixtures through castling, en passant, underpromotion, mate and stalemate. Open a promotion dialog over the board and prove its click is consumed. Restart and close cleanly. |
| P5. Dragging and hero-view selection | Deliver G3 and game drag policy using the engine gesture contract. Test thresholds, published-generation picking, release-outside and focus loss. | Drag a tall piece by its silhouette, orbit with the right button, resize, cancel a drag and complete a legal drop. Repeat in overhead view. Board history must contain exactly one move per committed drop. |
| P6. Menus and local persistence | Deliver G4 and G6 in separate engine proposals, then title/new-game/settings screens in the game. Add G5 when editable names or cross-display scaling require it; fixed slot names avoid making text entry a first-play blocker. | Navigate title to local play using keyboard and mouse, change a setting, save, close and relaunch. Continue the same game with castling/repetition state intact; exercise an unwritable location and see the error in-window. |
| P7. First opponent | Deliver G7, then the initial deterministic search and one middle ladder profile. Tests cover legal output, terminal states, mate-score orientation, transposition correctness, cancellation and bounded queues before adding personality. | Play a complete short game against the bot while orbiting/opening menus during thought. Undo and restart during search, then verify no stale reply enters the new game. Close during thought and verify worker teardown. |
| P8. Ladder and explanations | Add measured profiles and typed tactical facts/templates. Tests reproduce seeds, compare adjacent strengths and reject explanations whose line or premise is invalid. | Play at beginner and strongest validated settings; request each hint stage and replay a verified tactical explanation. Use blind human sessions to assess coherent mistakes, recording uncertainty rather than assigning unsupported ratings. |
| P9. First course and puzzle trainer | Author roughly a dozen lessons across rules, basic mate, hanging pieces, fork/pin and opening principles. Add G9/G10 incrementally when prose/annotations need them. Test schema validation, prerequisite cycles, all accepted branches, alternative mates and the CC0 CSV setup move. | From a fresh offline install, enter the browser, finish a lesson, fail and retry a task, use hints, solve a puzzle, exit and resume progress. The board and text must remain readable after resize. |
| P10. Own-game review and finishing services | Add background neutral analysis, replay/try-again and lesson recommendations, with explicit uncertainty. Deliver G8 sound and G11 accessibility as independent engine proposals; neither should be hidden inside chess review code. | Finish a game, open review, replay a mistake, try the better continuation and return to the recorded line without changing the result. Check sound/mute and screen-reader navigation in the real window when their services land. |

Perft fixtures are necessary but insufficient. Rules tests also need illegal FEN rejection, duplicate/missing kings in game imports, king adjacency, double check, pinned en-passant, castling after a rook has moved back, promotion captures and rights after rook capture. Search tests must cover a draw available through history rather than placement alone. Lesson tests must explore every authored branch, including moves that are legal but fail the objective.

For teaching acceptance, have learners explain the tactic or plan on an unseen position after using the lesson. Count independent success separately from success after a full hint. A completion animation and a higher puzzle score do not establish that the learner understood the idea.

## 9. Decisions and remaining uncertainty

Proceed with standard untimed chess, English instruction, local two-player play, eight proposed opponent identities, no account, bundled lessons/puzzles, optional sound and an original Rust search engine. The first teaching release prioritizes rules, tactical safety and basic mates; advanced strategy remains course expansion. No genuine product choice needs to block Andrew's approval of the first playable slice.

The unresolved items are evidence and implementation work: measured opponent strength, whether learners find the weakness model believable, exact dead-position coverage, GPU/UI composition behavior on each supported platform, and the license of any future model artifact. They have tests or explicit release limits above. A later decision to prioritize expert-level analysis, online competition or multilingual launch would change scope and should be made as a separate product decision.
