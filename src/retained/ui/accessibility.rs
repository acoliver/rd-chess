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
}
