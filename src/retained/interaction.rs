use super::BoardDriver;
use crate::{
    Square,
    rules::{LegalMove, PromotionPiece},
};
use std::collections::BTreeSet;
use worldrunner::{
    scene::{
        gpu::SceneView,
        viewer::{RetainedInput, RetainedPickHit, SceneObjectEdit},
    },
    simulation::{InputAction, PointerButton, PointerEvent, PointerWheel},
    ui::UiKey,
};

#[derive(Default)]
pub(super) struct Interaction {
    pub selected: Option<Square>,
    pub promotion: Vec<LegalMove>,
    pub cursor: Option<[f64; 2]>,
    press: Option<Press>,
}
struct Press {
    origin: [f64; 2],
    previous: [f64; 2],
    dragged: bool,
    click_allowed: bool,
}
impl Interaction {
    pub fn cancel_pointer(&mut self) {
        self.press = None;
        self.cursor = None;
    }
    fn motion(&mut self, point: [f64; 2]) -> Option<[f64; 2]> {
        self.cursor = Some(point);
        let press = self.press.as_mut()?;
        let distance = (point[0] - press.origin[0]).hypot(point[1] - press.origin[1]);
        if !press.dragged && distance <= 6. {
            return None;
        }
        let previous = if press.dragged {
            press.previous
        } else {
            press.origin
        };
        press.dragged = true;
        press.previous = point;
        Some([point[0] - previous[0], point[1] - previous[1]])
    }
    fn button(&mut self, pressed: bool, click_allowed: bool) -> Option<[f64; 2]> {
        if pressed {
            if self.press.is_none() {
                self.press = self.cursor.map(|point| Press {
                    origin: point,
                    previous: point,
                    dragged: false,
                    click_allowed,
                });
            }
            None
        } else {
            self.press
                .take()
                .filter(|press| !press.dragged && press.click_allowed && click_allowed)
                .and(self.cursor)
        }
    }
}

impl BoardDriver {
    pub fn selected(&self) -> Option<Square> {
        self.interaction.selected
    }
    pub fn promotion_pending(&self) -> bool {
        !self.interaction.promotion.is_empty()
    }
    pub fn destinations(&self) -> BTreeSet<Square> {
        let Some(board) = &self.board else {
            return BTreeSet::new();
        };
        board
            .game()
            .legal_moves()
            .into_iter()
            .filter(|chess_move| Some(chess_move.from()) == self.selected())
            .map(LegalMove::to)
            .collect()
    }
    pub fn status_text(&self) -> String {
        let error = self.storage.as_ref().and_then(|s| s.error.as_ref());
        let pgn = self.pgn.message.clone().or_else(|| {
            matches!(
                self.screen(),
                super::Screen::PgnImport | super::Screen::PgnGames
            )
            .then(|| self.pgn_location_text())
        });
        if let Some(message) = pgn {
            return error.map_or_else(
                || message.clone(),
                |error| {
                    format!(
                        "{message}
Storage error: {error}"
                    )
                },
            );
        }
        if let Some(error) = error {
            return format!("Storage error: {error}");
        }
        if let Some(result) = &self.menus.result {
            return result.clone();
        }
        if let Some(status) = self.computer_status() {
            return status;
        }
        self.board.as_ref().map_or_else(String::new, |board| {
            super::status::status_text(board.game())
        })
    }
    pub fn exit_requested(&self) -> bool {
        self.exit
    }

    /// Routes synthetic and OS observations identically, with a host hit from the presented frame.
    pub fn handle_input(
        &mut self,
        input: &RetainedInput,
        view: Option<&SceneView>,
        hit: Option<RetainedPickHit>,
    ) -> Result<bool, String> {
        if let Some(view) = view {
            self.viewport = [view.viewport().width, view.viewport().height];
        }
        match input {
            RetainedInput::Focused(focused) => self.unfocused = !focused,
            RetainedInput::Resized(size) => {
                self.viewport = *size;
                self.view = None;
            }
            _ => {}
        }
        if matches!(
            input,
            RetainedInput::PointerCancelled
                | RetainedInput::Focused(false)
                | RetainedInput::CursorLeft
                | RetainedInput::Resized(_)
        ) {
            self.interaction.cancel_pointer();
            self.route_pointer(PointerEvent::Cancel)?;
            return Ok(false);
        }
        if self.unfocused {
            return Ok(false);
        }
        self.sync_menu_ui()?;
        if let RetainedInput::Key {
            key,
            pressed,
            repeat,
            ..
        } = input
        {
            return self.key_input(key, *pressed, *repeat);
        }
        if let Some(InputAction::Pointer(pointer)) = input.simulation_action() {
            if self.route_pointer(pointer)? {
                self.interaction.cancel_pointer();
                return Ok(true);
            }
            self.board_pointer(pointer, view, hit)?;
        }
        Ok(false)
    }

    fn route_pointer(&mut self, pointer: PointerEvent) -> Result<bool, String> {
        if self.viewport.contains(&0) {
            return Ok(false);
        }
        let Some(ui) = &mut self.ui else {
            return Ok(false);
        };
        let response = ui
            .pointer(pointer, self.viewport)
            .map_err(|e| e.to_string())?;
        self.ui_actions(&response.actions, true)?;
        Ok(response.consumed)
    }

    fn board_pointer(
        &mut self,
        pointer: PointerEvent,
        view: Option<&SceneView>,
        hit: Option<RetainedPickHit>,
    ) -> Result<(), String> {
        match pointer {
            PointerEvent::Moved(point) => {
                if let Some(delta) = self.interaction.motion(point)
                    && let Some(camera) = &mut self.camera
                {
                    camera.orbit.drag(
                        delta,
                        worldrunner::scene::gpu::Viewport {
                            width: self.viewport[0],
                            height: self.viewport[1],
                        },
                    );
                    camera.dirty = true;
                }
            }
            PointerEvent::Button {
                button: PointerButton::Left,
                pressed,
            } => {
                let click_allowed = !self.computer_turn()
                    && !self.presenting
                    && self.pending.borrow().is_empty()
                    && hit.is_none_or(|picked| picked.generation == self.generation);
                if let Some(point) = self.interaction.button(pressed, click_allowed)
                    && let Some(view) = view
                {
                    self.click_square(self.clicked_square(view, point, hit))?;
                }
            }
            PointerEvent::Wheel(delta) => {
                if let Some(camera) = &mut self.camera {
                    camera.orbit.zoom(match delta {
                        PointerWheel::Lines(delta) => delta[1],
                        PointerWheel::Pixels(delta) => delta[1] / 40.,
                    });
                    camera.dirty = true;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn click_square(&mut self, square: Option<Square>) -> Result<(), String> {
        let Some(board) = &self.board else {
            return Ok(());
        };
        if self.game_ended() {
            return Ok(());
        }
        let Some(square) = square else {
            return self.clear_selection();
        };
        if self.selected() == Some(square) {
            return self.clear_selection();
        }
        if board
            .game()
            .piece_at(square)
            .is_some_and(|(side, _)| side == board.game().side_to_move())
        {
            self.interaction.selected = Some(square);
            return Ok(());
        }
        let moves: Vec<_> = board
            .game()
            .legal_moves()
            .into_iter()
            .filter(|chess_move| {
                Some(chess_move.from()) == self.selected() && chess_move.to() == square
            })
            .collect();
        match moves.as_slice() {
            [] => Ok(()),
            [chess_move] => self.commit_move(*chess_move),
            _ => {
                self.interaction.promotion = moves;
                self.ui
                    .as_mut()
                    .expect("configured game UI")
                    .show_dialog(true)
                    .map_err(|e| e.to_string())
            }
        }
    }

    pub(super) fn promote(&mut self, index: usize) -> Result<(), String> {
        let promotion = [
            PromotionPiece::Queen,
            PromotionPiece::Rook,
            PromotionPiece::Bishop,
            PromotionPiece::Knight,
        ][index];
        let chess_move = self
            .interaction
            .promotion
            .iter()
            .find(|chess_move| chess_move.promotion() == Some(promotion))
            .copied()
            .ok_or("promotion choice without pending move")?;
        self.commit_move(chess_move)
    }

    pub(super) fn commit_move(&mut self, chess_move: LegalMove) -> Result<(), String> {
        let edits = self
            .board
            .as_mut()
            .expect("playable board")
            .play(chess_move)
            .map_err(|e| e.to_string())?;
        self.enqueue_move(edits)
    }

    pub(super) fn enqueue_move(&mut self, mut edits: Vec<SceneObjectEdit>) -> Result<(), String> {
        self.pgn.message = None;
        self.computer_changed();
        self.clear_selection()?;
        edits.extend(self.markers.clear());
        self.pending.borrow_mut().push_back(edits);
        self.refresh_game_over()?;
        self.autosave();
        self.sync_menu_ui()
    }
    pub(super) fn clear_selection(&mut self) -> Result<(), String> {
        self.interaction.selected = None;
        self.interaction.promotion.clear();
        self.interaction.cancel_pointer();
        if let Some(ui) = &mut self.ui {
            ui.show_dialog(false).map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn key_input(&mut self, key: &str, pressed: bool, repeat: bool) -> Result<bool, String> {
        if let Some(ui_key) = ui_key(key)
            && let Some(ui) = &mut self.ui
            && !self.viewport.contains(&0)
        {
            let response = ui
                .key(ui_key, pressed, repeat, self.viewport)
                .map_err(|e| e.to_string())?;
            self.ui_actions(
                &response.actions,
                matches!(ui_key, UiKey::Enter | UiKey::Space),
            )?;
            if response.consumed {
                return Ok(true);
            }
        }
        if !pressed || repeat {
            return Ok(false);
        }
        let undo_finished =
            self.screen() == super::Screen::GameOver && matches!(key, "u" | "backspace");
        if self.screen() != super::Screen::Play && key != "escape" && !undo_finished {
            return Ok(true);
        }
        match key {
            "escape" if self.promotion_pending() || self.selected().is_some() => {
                self.clear_selection()?;
            }
            "escape"
                if matches!(
                    self.screen(),
                    super::Screen::PgnImport | super::Screen::PgnGames
                ) =>
            {
                self.pgn_back()?;
            }
            "escape" if self.board.is_some() => {
                let screen = match self.screen() {
                    super::Screen::Play => super::Screen::InGameMenu,
                    super::Screen::InGameMenu => super::Screen::Play,
                    super::Screen::ComputerSetup | super::Screen::Settings => super::Screen::Title,
                    super::Screen::Slots => self.menus.parent,
                    screen => screen,
                };
                self.transition(screen)?;
            }
            "escape" => self.exit = true,
            "u" | "backspace" if self.board.is_some() => {
                self.clear_selection()?;
                if self.undo_plies().is_ok() {
                    self.undo()?;
                }
            }
            "r" if self.board.is_some() => {
                self.restart()?;
                self.reset_camera();
            }
            "home" | "r" => self.reset_camera(),
            _ => {}
        }
        Ok(false)
    }
    pub(super) fn reset_camera(&mut self) {
        self.interaction.cancel_pointer();
        if let Some(camera) = &mut self.camera {
            camera.orbit.reset();
            camera.dirty = true;
        }
    }
}

fn ui_key(key: &str) -> Option<UiKey> {
    match key {
        "arrowup" | "up" => Some(UiKey::Up),
        "arrowdown" | "down" => Some(UiKey::Down),
        "home" => Some(UiKey::Home),
        "end" => Some(UiKey::End),
        "tab" => Some(UiKey::Tab),
        "shift" => Some(UiKey::Shift),
        "enter" => Some(UiKey::Enter),
        "space" => Some(UiKey::Space),
        _ => None,
    }
}
