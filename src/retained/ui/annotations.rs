use super::{GameUi, UiOverlay, UiRenderPlan};
use std::{collections::BTreeMap, sync::Arc};
use worldrunner::{
    scene::gpu::SceneView,
    ui::plan::ui::{
        UiPresentation,
        annotations::{Annotation, extract_annotations},
    },
};

impl GameUi {
    pub(in crate::retained) fn annotation_font(&self) -> worldrunner::ui::FontId {
        self.native.font
    }
    pub(in crate::retained) fn set_annotations(
        &mut self,
        annotations: &[Annotation],
        view: &SceneView,
    ) -> Result<(), String> {
        let draws = extract_annotations(annotations, &self.native.fonts, view.resolved_view())
            .map_err(|e| e.to_string())?;
        if draws != self.annotation_draws {
            self.annotation_draws = draws;
            self.dirty = true;
        }
        Ok(())
    }
    pub(super) fn publish_plan(
        &mut self,
        plan: UiRenderPlan,
    ) -> Result<Option<Arc<UiOverlay>>, String> {
        let mut presentation = UiPresentation::from_plan(&plan);
        presentation
            .append(self.annotation_draws.clone())
            .map_err(|e| e.to_string())?;
        self.plan = Some(plan);
        if self.presentation.as_ref() == Some(&presentation) {
            return Ok(None);
        }
        let overlay = UiOverlay::from_presentation(&presentation, &BTreeMap::new())
            .map_err(|e| e.to_string())?;
        self.presentation = Some(presentation);
        Ok(Some(Arc::new(overlay)))
    }
}
