use super::{BoardDriver, ui};
use crate::{
    Square,
    rules::{GameStatus, LegalMove, PromotionPiece},
};
use std::collections::BTreeSet;
use worldrunner::{
    scene::{
        gpu::SceneView,
        viewer::{RetainedInput, SceneObjectEdit},
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
        self.board.as_ref().map_or_else(String::new, |board| {
            super::status::status_text(board.game())
        })
    }
    pub fn exit_requested(&self) -> bool {
        self.exit
    }

    /// Routes synthetic and OS observations identically. The view is the published physical viewport.
    pub fn handle_input(
        &mut self,
        input: &RetainedInput,
        view: Option<&SceneView>,
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
            self.board_pointer(pointer, view)?;
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
        if let Some(index) = ui::choice(&response.actions) {
            self.promote(index)?;
        }
        Ok(response.consumed)
    }

    fn board_pointer(
        &mut self,
        pointer: PointerEvent,
        view: Option<&SceneView>,
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
                let click_allowed = !self.presenting && self.pending.borrow().is_empty();
                if let Some(point) = self.interaction.button(pressed, click_allowed)
                    && let Some(view) = view
                {
                    self.click_square(pick_square(view, point))?;
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
        if !matches!(board.game().status(), GameStatus::Ongoing { .. }) {
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

    fn promote(&mut self, index: usize) -> Result<(), String> {
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

    fn commit_move(&mut self, chess_move: LegalMove) -> Result<(), String> {
        let edits = self
            .board
            .as_mut()
            .expect("playable board")
            .play(chess_move)
            .map_err(|e| e.to_string())?;
        self.enqueue_move(edits)
    }

    pub(super) fn enqueue_move(&mut self, mut edits: Vec<SceneObjectEdit>) -> Result<(), String> {
        self.clear_selection()?;
        edits.extend(self.markers.clear());
        self.pending.borrow_mut().push_back(edits);
        Ok(())
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
            if let Some(index) = ui::choice(&response.actions) {
                self.promote(index)?;
            }
            return Ok(response.consumed);
        }
        if !pressed || repeat {
            return Ok(false);
        }
        match key {
            "escape" if self.promotion_pending() || self.selected().is_some() => {
                self.clear_selection()?;
            }
            "escape" => self.exit = true,
            "u" | "backspace" if self.board.is_some() => {
                self.clear_selection()?;
                if self
                    .board
                    .as_ref()
                    .expect("game")
                    .game()
                    .repetition_history()
                    .len()
                    > 1
                {
                    self.undo()?;
                }
            }
            "r" if self.board.is_some() => {
                let edits = self.board.as_mut().expect("game").restart();
                self.enqueue_move(edits)?;
                self.reset_camera();
            }
            "home" | "r" => self.reset_camera(),
            _ => {}
        }
        Ok(false)
    }
    fn reset_camera(&mut self) {
        self.interaction.cancel_pointer();
        if let Some(camera) = &mut self.camera {
            camera.orbit.reset();
            camera.dirty = true;
        }
    }
}

fn ui_key(key: &str) -> Option<UiKey> {
    match key {
        "tab" => Some(UiKey::Tab),
        "shift" => Some(UiKey::Shift),
        "enter" => Some(UiKey::Enter),
        "space" => Some(UiKey::Space),
        _ => None,
    }
}
fn pick_square(view: &SceneView, point: [f64; 2]) -> Option<Square> {
    let viewport = view.viewport();
    if point[0] < 0.
        || point[1] < 0.
        || point[0] >= f64::from(viewport.width)
        || point[1] >= f64::from(viewport.height)
    {
        return None;
    }
    let ray = view
        .resolved_view()
        .screen_ray(glam::DVec2::from_array(point).as_vec2())
        .ok()?;
    let hit = ray
        .intersect_plane(glam::Vec3::new(0., 0., 0.05), glam::Vec3::Z)
        .ok()?;
    if !(-4. ..4.).contains(&hit.x) || !(-4. ..4.).contains(&hit.y) {
        return None;
    }
    Square::new((hit.x + 4.).floor() as u8, (hit.y + 4.).floor() as u8).ok()
}
