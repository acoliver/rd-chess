# First course: original exercises and prose

`first-course.json` is a version-1 game-owned course. Its 12 lessons contain
19 board tasks plus short explanations. The prose and elementary positions were
authored for rd-chess, without copying book text, problems or annotations. They
retain the repository's existing licence. CC0 puzzle data is separately noticed
in `../puzzles/README.md`.

The course covers movement, check, supported queen mate, rook ladder mate,
back-rank mate, hanging pieces, counting attackers and defenders, forks, pins,
skewers, opening principles and mixed practice. Pawn promotion and discovered
attacks/checks have short explanations in first moves and mixed practice.
It is a first course, not a
complete rules reference or proof of playing strength. Specific-line tasks name
the requested exercise rather than declaring every other legal move bad.

## Format and validation

The top-level object has `version` and `lessons`. Each lesson has `id`, `title`,
`requires` (prerequisite IDs), optional typed `motifs`, optional `terms`, and ordered `steps`.
`terms` lists exact glossary aliases taught by the lesson. Aliases are printable
ASCII, at most 64 bytes, unpadded and unique across the course without regard to
case. The native UI links whole-word matches, preferring longer phrases; it also
recognises lesson IDs and titles in cross-references. All displayed text remains
unchanged. Lesson headings are excluded from linking.
Motif tags use the `teaching::explain::Motif` names in snake_case. Unknown or
duplicate tags fail validation. `Course::related_lesson` selects the first lesson
with the requested tag in authored order; absent tags return no lesson. Review
uses this mapping instead of guessing a lesson from its title or a score.
The window browser locks lessons until their prerequisites are complete; CPU
runners still allow starting any validated lesson.

A step has `kind: "explain"`, `text` and optional `annotations`, or
`kind: "task"` with these fields:

- `text`, six-field `fen`, and `side` (`ivory` means white, `onyx` means black);
- `goal`: `line`, `checkmate` or `any_mate`;
- `solutions`: UCI move nodes `{ "uci": "e2e4", "next": [...] }`;
- `hints`, ordered from a question or observation to the explicit move/line;
- `feedback`, mapping common wrong UCI moves to authored explanations;
- optional `annotations` with `squares` (algebraic names), `square_colour`
  (linear RGBA), and `arrows` (`{ "from": "e1", "to": "e7", "colour":
  [0.0, 0.7, 0.8, 1.0] }`). Colours default to cyan when omitted and must
  contain four finite components in 0..1. The window publishes native rings
  and arrows at square centres, 65 mm above the board origin. World anchors
  use the engine's depth-tested annotations, so foreground pieces occlude them.
  Annotation widths and ring radii are physical pixels, as defined by the engine.

The solution tree starts on the solver's turn. Sibling solver nodes are accepted
alternatives. Their children contain exactly one scripted opponent reply, whose
children are the next solver choices. A leaf must follow a solver move. Duplicate
siblings, branching opponent replies and continuations after terminal states are
unreachable and rejected. The runner chooses no unrecorded opponent move.
`any_mate` also accepts a legal checkmate outside the tree; `checkmate` validates
mate at every leaf but keeps the explicitly requested line.

`Course::from_json` validates the entire course before returning it. Unknown
fields and versions, duplicate IDs, missing prerequisites, cycles, invalid FENs,
side mismatches, illegal lines, unreachable branches and false mate goals fail
with lesson and step context. FEN legality has the rules core's documented local
validation scope; it is not a retrograde proof. Trees are limited to 32 plies and
2048 nodes per task; courses to 128 lessons and 64 steps per lesson. Course JSON
is bounded to 256 KiB. Printable ASCII English text is used for the first UI.
Titles allow 120 bytes, explanation paragraphs 1500, task text 1000, and each
hint or feedback entry 500. All must contain non-whitespace text. Every task
requires one to eight hints. Square names and arrow endpoints are validated.

## CPU runners and progress

`TaskRunner` reports `AwaitingMove`, `Correct` (a nonfinal correct solver turn),
`Wrong(feedback)` or `Completed`. A correct submission applies the solver move
and scripted reply together. A wrong submission leaves the board and branch
unchanged. Hint levels advance until exhausted. `LessonRunner` also exposes the
current explanation/task, prevents skipping unfinished tasks, and resets the
board and hint level on retry. Advancing past the final step marks the lesson
complete. No view, renderer or search worker is involved.

`Progress` is a versioned JSON save payload containing completed lesson IDs,
per-lesson completed-step indices, submitted-move attempts and revealed hints,
and puzzle rating-band
submitted-move attempts/solve counts. Invalid and wrong answers count as attempts;
opening or retrying a task does not. Repeated hint requests at the cap do not
increase usage. Starting a runner with `with_progress` preserves earlier totals;
it starts the chosen lesson/puzzle at its initial checkpoint. The payload does
not save a half-finished task or choose a file location. The window persists it
through the engine's transactional `ApplicationStore`. Solve counts include
repeated solves.

```sh
cargo run --locked -p chess3d -- --lesson rules.first-moves
cargo run --locked -p chess3d -- --lesson openings.reasons
cargo test --locked -p chess3d --test teaching_course
```

CLI inspection validates the bundled course and prints the chosen lesson's
steps, start FENs, accepted lines and hints without creating a graphics device.
The tests walk every authored solution branch, check alternative mates, wrong
feedback, retry/hint state and progress round trips.

## Window controls and storage

A versioned progress payload records completed steps and lessons, attempts, hints
and rating-band totals. In the window, title choices open Lessons and Puzzles.
The native lesson list scrolls, marks completed lessons and disables rows whose
prerequisites are unfinished. Enter or click opens an unlocked lesson. Next and
Back navigate its explanations and board tasks; Hint reveals the authored hints
in order, and Retry restores the task checkpoint. Accepted mouse moves play the
scripted reply in the same retained object transaction. Wrong answers leave the
checkpoint unchanged and show the authored feedback. Native rings show authored
square highlights in `square_colour`; native arrows use each arrow's `colour`.
Changing steps replaces the annotation set rather than retaining old marks.

Puzzles offers all bands or a chosen rating band, with solved and submitted-move
totals. Its setup position is presented before a 350 ms retained-tick delay applies
the opponent's setup move. The solver then plays by mouse. Puzzles may supply
an optional `annotations` object with the same fields as lesson steps; it is
shown after setup. In both lessons and puzzles, the second Hint request rings
the current solver piece and the third adds its move arrow. The move comes from
the current solution checkpoint, including after a scripted reply. Authored hint
text still stops at its own cap. Successful moves and Retry clear hint marks;
Retry restores the post-setup position, and Next chooses the next matching puzzle.
Any legal mate is accepted in mating puzzles.
Wrong legal moves also show verified explanation sentences when facts are found.
The study camera leaves space for one wrapped text panel below the board. Escape
returns to the browser, then the title; leaving study restores the play camera.

Progress loads at startup from the `training-progress` record in the same engine
`ApplicationStore` used by saved games. Changes to completed steps and lessons,
attempts, hints and puzzle totals are written immediately. Both the record
metadata and JSON payload use version 1. Store, encoding, version and payload
failures have typed `ProgressError` variants; damaged records are reported and
preserved rather than replaced. Manual game slots and Continue remain usable
when the progress record is corrupt. Training checkpoints never replace a game
autosave. There is no separate progress-file writer.

After a computer reply, the game and game-over menus' Why? resumes play with
learner sentences for that reply. When analysis supplies an alternative or a
verified adverse exchange, it also draws labelled Better and Threat arrows.
Selected review moments show the searched better move and the first refutation
move. `assets/annotations.json` defines blue (#0072B2) suggestions and vermilion
(#D55E00) threats, stored as linear RGBA and separate from lesson authors' colours.
Marks clear on a move, undo or leaving their screen; review marks appear only at
the selected moment's cursor.

Synthetic driver tests exercise navigation, locks, authored and wrong moves,
scripted replies, hint caps, checkpoints, completion/unlocking, setup timing,
alternative mates and explanations. The required Metal/lavapipe teaching capture
checks stable native list pixels and independently counts cyan pixels around
the authored e3 ring, with negative controls before highlighting and around adjacent
d3. Another windowless capture counts authored cyan arrow pixels along the
projected g1-f3 line, rejects matching pixels elsewhere, and checks that Next
clears the arrow.
The Xvfb smoke opens Puzzles and plays 00008's first solver move. Its click centers
come from the same study pose and engine projection, printed by viewer-enabled
`--puzzle` inspection; it is not run as a visible macOS window.


Verification for the screen work: formatting, strict macOS default/all-feature
Clippy and Linux all-feature Clippy through Zig passed with rc=0. CPU checks
passed 122 library and 90 integration cases. The full all-feature Metal run
through the shared AquaJob passed 225 tests with no ignored cases; its only
filter was the release-only self-play regression, which passed separately with
six wins. Python tooling passed 62 tests, and the Xvfb smoke passed syntax
compilation but was not executed on macOS. The title-to-Lessons regression was
observed failing before implementation. Capture/log evidence is under
`target/train-verify/`; independent cyan-square assertions passed on Metal.
