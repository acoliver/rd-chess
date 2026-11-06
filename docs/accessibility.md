# Accessible chess controls

Chess publishes game-owned names and move descriptions through the engine's
native UI router. The engine owns semantic-tree publication and its AccessKit /
VoiceOver connection. The game makes no platform accessibility calls and keeps
no separate semantic tree.

## Board navigation

The board is a group named `Chess board` containing 64 button controls. Names
include the coordinate and occupant, such as `e4, white knight` or `d5, empty`.
Selection adds `selected`; legal targets add `legal destination`. With White's
orientation the reading order is a8 through h8, then a7 through h7, ending at h1.
With Black's orientation it is h1 through a1, ending at a8. Local games use White's
orientation; computer games use the human's chosen side and rotate the camera
preset to match. Orbiting changes the view, not the chosen reading order.

Arrow keys move one square in that orientation and stop at an edge. Without
existing focus, an arrow starts from the first square and applies its direction.
Enter and Space select or move. Tab enters the board at an edge of its tab order;
once inside, Tab or Shift-Tab leaves the board for other controls when available.
Assistive technology can focus and press any square directly. Keyboard repeat
moves focus but does not repeat activation. A gold border follows native focus.

Selection and moves use the same game input handler as a click. Computer turns,
modal dialogs and unpublished board edits prevent activation; squares remain
available for inspection during a computer turn. Pointer picking still uses the
last published 3D frame, including piece silhouettes. Transparent semantic square
controls do not intercept clicks or add retained scene-object transactions.

## Announcements and dialogs

A live Status node announces committed moves in plain words. For example:
`White knight from g1 to f3`. Captures name the captured piece. Castling names the
king and rook paths; promotions name the new piece; en passant identifies the
capture type. Check, checkmate and automatic draws follow the move description.
The same path describes computer replies. Undo, restart and loading clear the
previous game's transient announcement rather than repeating an old result.

The live node belongs to the active dialog when one is open, so announcements
remain inside the engine's modal focus restriction. Promotion choices, menu
lists, saved-game actions and the save-label field have accessible names.
Resignation opens `Resign game?` with Cancel first and Confirm resign second.
Enter on the initial choice cancels. Escape also cancels.

Offer draw is available in the game menu. Local play announces which side offered
and presents Decline draw first and Accept draw second. Escape declines. Agreement
ends the game with `Draw by agreement`. The computer currently declines all offers
with `Computer declined the draw offer`; rule-based draw claims are unchanged.

## Engine limitations and verification scope

The current engine exposes editable text as role Text, with Focus as its only
semantic action. Its semantic value cannot represent editable text, caret or
selection, and it provides no semantic text-editing requests. The save-label
control has an explicit name and remains keyboard-editable, but the game cannot
supply those missing editing semantics through this engine API.

Board names, focus, activation, modal exclusion, render-plan focus borders and
live-announcement wording have CPU regressions. Those checks do not establish
that a particular hardware screen-reader configuration speaks or edits correctly;
that requires native application testing. GPU tests compile separately from their
execution, and CPU checks do not establish the rendered border's pixel output.
