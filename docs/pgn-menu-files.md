# PGN menu files

Export PGN appears beside Save / Load in the pause menu and on the game-over
screen. It exports the current game's complete main line and result, including
SetUp/FEN for a nonstandard start. The roster uses Event `Casual game`, Site
`rd-chess`, the system clock's UTC date, Round `-`, and `Player` or the named
computer profile for each side. Status shows the written path.

Files go in `exports/` under the same application-data root as the saves,
including an explicit `--data-dir` override. The default on macOS is
`~/Library/Application Support/org.acoliver.chess3d/exports/`. Names have the
form `game-YYYYMMDD-HHMMSSZ-0001.pgn`. An exclusive-create counter prevents
repeated exports, including exports in the same second, from overwriting files.
The pinned engine supplies only the application-data location, with no documents
or export-directory service, so PGN files stay next to the saves.

Saved games has an Import PGN choice. Its screen shows the drop directory and
lists regular `.pgn` files in filename order (extension matching ignores case).
Drop files there, then reopen Import PGN to refresh the list. A multi-game file
opens a second list to choose the game. Arrows navigate, Enter opens, and Escape
or Back returns to the previous screen. Empty files, non-UTF-8 data, oversized
input and parser errors appear in status without replacing the live game.
Import names cannot contain directory components; symlink directories and files
are refused.

The chosen main line loads at its final position as a local two-player game,
with its declared result and complete undo history. U/Backspace also works from
the game-over screen, clearing the result when a move is undone. The game
autosaves and can be saved into a numbered slot. Comments and variations are
ignored by the playable game, as noted on the import screen and in its completion
message. This pin has
no native file dialog or clipboard service. Copy PGN is omitted while the engine
clipboard proposal is pending; no platform-specific clipboard code is added here.

`tests/pgn_menus.rs` drives native menus with synthetic pointer and keyboard input
without a GPU. It checks export metadata and round trips, collision preservation,
single and multi-game import, parser diagnostics, save/reload and undo, scrolling,
Back/refresh, and path and symlink rejection. UTC calendar and same-second export
counter tests run in `retained::pgn_files`.
