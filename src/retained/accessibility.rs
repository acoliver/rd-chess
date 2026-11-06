use super::{BoardDriver, Screen, ui::BoardState};
use worldrunner::scene::gpu::{SceneView, Viewport};

impl BoardDriver {
    pub(super) fn board_input_allowed(&self) -> bool {
        (self.retry_active() || !self.computer_turn())
            && (self.screen() == Screen::Play || self.training_screen() || self.retry_active())
            && !self.training_blocked()
            && !self.presenting
            && self.pending.borrow().is_empty()
            && !self.promotion_pending()
            && (!self.game_ended() || self.retry_active())
            && !self.unfocused
    }
    pub(super) fn sync_accessibility(&mut self) -> Result<(), String> {
        let Some(board) = &self.board else {
            return Ok(());
        };
        let orientation = self.board_orientation();
        let destinations = self.destinations();
        let labels = (0..64)
            .map(|index| {
                let square = super::announcements::oriented_square(index, orientation);
                let mut label = super::announcements::square_label(board.game(), square);
                if self.selected() == Some(square) {
                    label.push_str(", selected");
                }
                if destinations.contains(&square) {
                    label.push_str(", legal destination");
                }
                label
            })
            .collect();
        let state = BoardState {
            labels,
            orientation,
            visible: matches!(
                self.screen(),
                Screen::Play | Screen::Lesson | Screen::Puzzle
            ) || self.retry_active(),
        };
        let announcement = self
            .interaction
            .announcement
            .clone()
            .or_else(|| self.menus.result.clone())
            .or_else(|| board.announcement.clone())
            .unwrap_or_else(|| self.status_text());
        let view = self.accessibility_view()?;
        if let Some(ui) = &mut self.ui {
            ui.synchronize_announcement(announcement)
                .map_err(|e| e.to_string())?;
            if let Some(view) = view {
                ui.synchronize_board(&state, &view)
                    .map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    fn accessibility_view(&self) -> Result<Option<SceneView>, String> {
        if let Some(view) = &self.view {
            return Ok(Some(*view));
        }
        let (Some(board), Some(pose)) = (&self.board, self.camera_pose()) else {
            return Ok(None);
        };
        if self.viewport.contains(&0) {
            return Ok(None);
        }
        let camera = *board.scene().effects().expect("chess camera").camera();
        SceneView::new(
            pose,
            camera,
            Viewport {
                width: self.viewport[0],
                height: self.viewport[1],
            },
        )
        .map(Some)
        .map_err(|e| e.to_string())
    }
    pub(super) fn semantic_update(
        &mut self,
    ) -> Result<Option<worldrunner::ui::UiSemanticUpdate>, String> {
        if self.viewport.contains(&0) {
            return Ok(None);
        }
        self.sync_menu_ui()?;
        self.ui
            .as_mut()
            .map(|ui| ui.semantic_update(self.viewport).map_err(|e| e.to_string()))
            .transpose()
    }
}
