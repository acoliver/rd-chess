pub(super) fn list_label(entries: &[(&str, super::MenuAction)]) -> &'static str {
    use super::MenuAction;
    match entries.first().map(|(_, action)| action) {
        Some(MenuAction::Side(_)) => "Human side",
        Some(MenuAction::Level(_)) => "Computer level",
        Some(MenuAction::Markers(_)) => "Legal move markers",
        Some(MenuAction::Overhead(_)) => "Camera view",
        Some(MenuAction::Lesson(_)) => "Lessons",
        Some(MenuAction::PuzzleBand(_)) => "Puzzle difficulty",
        Some(
            MenuAction::Save(_)
            | MenuAction::Load(_)
            | MenuAction::Rename(_)
            | MenuAction::ReviewSaved(_),
        ) => "Saved game actions",
        Some(MenuAction::PgnEntry(_)) => "PGN games",
        Some(MenuAction::StudyBack) => "Lesson actions",
        Some(MenuAction::CancelDecision) => "Resignation choices",
        Some(MenuAction::DeclineDraw) => "Draw offer responses",
        _ => "Game actions",
    }
}

impl super::super::BoardDriver {
    pub(in crate::retained) fn accessibility_input(
        &mut self,
        request: worldrunner::ui::UiAccessRequest,
    ) -> Result<(), String> {
        if self.viewport.contains(&0) {
            return Ok(());
        }
        self.sync_menu_ui()?;
        if let Some(ui) = &mut self.ui {
            let response = ui
                .accessibility(request, self.viewport)
                .map_err(|e| e.to_string())?;
            self.ui_actions(&response.actions, true)?;
        }
        Ok(())
    }
}

use super::GameUi;
use worldrunner::{
    simulation::WorldError,
    ui::{UiAccessRequest, UiResponse, UiSemanticUpdate},
};

impl GameUi {
    pub fn accessibility(
        &mut self,
        request: UiAccessRequest,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.dirty = true;
        self.native
            .router
            .accessibility_action(&self.native.world, &layout, request)
    }

    pub fn semantic_update(&mut self, viewport: [u32; 2]) -> Result<UiSemanticUpdate, WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.native
            .router
            .semantic_update(&self.native.world, &layout)
    }
    pub(in crate::retained) fn synchronize_announcement(
        &mut self,
        text: String,
    ) -> Result<(), WorldError> {
        let state = self.state.as_ref().expect("synchronized UI");
        let parent = if state.promotion {
            self.dialog
        } else {
            self.pages
                .iter()
                .find(|(screen, _)| *screen == state.screen)
                .map_or(self.native.root, |(_, panel)| *panel)
        };
        self.native.world.set_parent(
            self.announcement,
            Some(parent),
            worldrunner::simulation::ParentSpace::KeepLocal,
        )?;
        self.native.router.semantics.insert(
            self.announcement,
            worldrunner::ui::UiSemantics {
                label: Some(text),
                live: true,
            },
        );
        Ok(())
    }
}
