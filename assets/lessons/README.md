# First course: original exercises and prose

`first-course.json` is a version-1 game-owned course. Its 12 lessons contain
19 board tasks plus short explanations. The prose and elementary positions were
authored for rd-chess, without copying book text, problems or annotations. They
retain the repository's existing licence. CC0 puzzle data is separately noticed
in `../puzzles/README.md`.

The course covers movement, check, supported queen mate, rook ladder mate,
back-rank mate, hanging pieces, counting attackers and defenders, forks, pins,
skewers, opening principles and mixed practice. It is a first course, not a
complete rules reference or proof of playing strength. Specific-line tasks name
the requested exercise rather than declaring every other legal move bad.

## Format and validation

The top-level object has `version` and `lessons`. Each lesson has `id`, `title`,
`requires` (prerequisite IDs) and ordered `steps`. Prerequisites are guidance for
a future browser, not access locks; runners allow starting any validated lesson.

A step has `kind: "explain"`, `text` and optional `annotations`, or
`kind: "task"` with these fields:

- `text`, six-field `fen`, and `side` (`ivory` means white, `onyx` means black);
- `goal`: `line`, `checkmate` or `any_mate`;
- `solutions`: UCI move nodes `{ "uci": "e2e4", "next": [...] }`;
- `hints`, ordered from a question or observation to the explicit move/line;
- `feedback`, mapping common wrong UCI moves to authored explanations;
- optional `annotations` with `squares` (algebraic names) and `arrows`
  (`{ "from": "e1", "to": "e7" }`). These are data for later engine UI work.

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
per-lesson submitted-move attempts and revealed hints, and puzzle rating-band
submitted-move attempts/solve counts. Invalid and wrong answers count as attempts;
opening or retrying a task does not. Repeated hint requests at the cap do not
increase usage. Starting a runner with `with_progress` preserves earlier totals;
it starts the chosen lesson/puzzle at its initial checkpoint. This payload does
not yet save a half-finished task or choose a file location. Transactional file
I/O and UI integration remain later work. Solve counts include repeated solves.

```sh
cargo run --locked -p chess3d -- --lesson rules.first-moves
cargo run --locked -p chess3d -- --lesson openings.reasons
cargo test --locked -p chess3d --test teaching_course
```

CLI inspection validates the bundled course and prints the chosen lesson's
steps, start FENs, accepted lines and hints without creating a graphics device.
The tests walk every authored solution branch, check alternative mates, wrong
feedback, retry/hint state and progress round trips.
