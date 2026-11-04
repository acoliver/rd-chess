mod pages;
#[cfg(test)]
mod readability_tests;
mod study;
pub(super) use study::StudyState;
mod widgets;
use super::menus::{self, MenuAction, Screen};
use glam::Vec2;
use std::{collections::BTreeMap, sync::Arc};
use widgets::NativeUi;
use worldrunner::{
    scene::gpu::simulation::UiOverlay,
    simulation::{ComponentDesc, ComponentHandle, ObjectHandle, PointerEvent, WorldError},
    ui::{
        RectTransform, UiAction, UiControl, UiKey, UiResponse,
        plan::ui::{UiRenderPlan, extract_ui_layout},
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct UiState {
    pub screen: Screen,
    pub promotion: bool,
    pub claim: bool,
    pub undo: bool,
}
pub(super) struct GameUi {
    native: NativeUi,
    study: Option<study::StudyWidgets>,
    study_state: Option<(Option<StudyState>, Screen)>,
    rows: BTreeMap<ObjectHandle, Vec<ObjectHandle>>,
    status: ComponentHandle,
    dialog: ObjectHandle,
    claim: ObjectHandle,
    undo: Option<ObjectHandle>,
    pages: Vec<(Screen, ObjectHandle)>,
    bindings: BTreeMap<ObjectHandle, Vec<MenuAction>>,
    state: Option<UiState>,
    cached: Option<(String, [u32; 2])>,
    plan: Option<UiRenderPlan>,
    dirty: bool,
}
impl GameUi {
    pub fn new() -> Result<Self, WorldError> {
        let mut native = NativeUi::new()?;
        let status = status_widget(&mut native)?;
        let claim = claim_widget(&mut native)?;
        let dialog = promotion_dialog(&mut native)?;
        let mut ui = Self {
            native,
            study: None,
            study_state: None,
            rows: BTreeMap::new(),
            status,
            dialog,
            claim,
            undo: None,
            pages: Vec::new(),
            bindings: BTreeMap::new(),
            state: None,
            cached: None,
            plan: None,
            dirty: true,
        };
        ui.build_pages()?;
        ui.build_study()?;
        Ok(ui)
    }
    pub fn synchronize(&mut self, state: UiState, viewport: [u32; 2]) -> Result<(), WorldError> {
        if self.state == Some(state) {
            return Ok(());
        }
        let changed = self.state.is_none_or(|old| old.screen != state.screen);
        for &(screen, object) in &self.pages {
            self.native
                .world
                .set_active(object, state.screen == screen)?;
        }
        self.native.world.set_active(self.claim, state.claim)?;
        self.show_dialog(state.promotion)?;
        if let Some(object) = self.undo {
            self.native
                .router
                .controls
                .get_mut(&object)
                .expect("undo control")
                .enabled = state.undo;
        }
        self.state = Some(state);
        self.dirty = true;
        if changed && state.screen != Screen::Play && !viewport.contains(&0) {
            self.focus_first(viewport)?;
        }
        Ok(())
    }
    fn focus_first(&mut self, viewport: [u32; 2]) -> Result<(), WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        for pressed in [true, false] {
            self.native
                .router
                .key(&self.native.world, &layout, UiKey::Tab, pressed, false)?;
        }
        Ok(())
    }
    pub fn show_dialog(&mut self, active: bool) -> Result<(), WorldError> {
        self.native.world.set_active(self.dialog, active)?;
        Ok(())
    }
    pub fn pointer(
        &mut self,
        event: PointerEvent,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.dirty = true;
        self.native
            .router
            .pointer(&self.native.world, &layout, event)
    }
    pub fn key(
        &mut self,
        key: UiKey,
        pressed: bool,
        repeat: bool,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        if self.native.router.focused().is_none()
            && self.state.is_some_and(|state| state.screen != Screen::Play)
        {
            self.focus_first(viewport)?;
        }
        let layout = self
            .native
            .world
            .ui_layout_with(viewport.into(), &self.native.router.layout)?;
        self.dirty = true;
        self.native
            .router
            .key(&self.native.world, &layout, key, pressed, repeat)
    }
    pub fn action(&self, action: UiAction) -> Option<MenuAction> {
        match action {
            UiAction::Activated { object } if object == self.claim => Some(MenuAction::ClaimDraw),
            UiAction::Selected { list, index } => Some(self.bindings[&list][index]),
            action => self.study_action(action),
        }
    }
    pub fn overlay(
        &mut self,
        text: &str,
        size: [u32; 2],
    ) -> Result<Option<Arc<UiOverlay>>, String> {
        let cached = (text.to_owned(), size);
        if self.cached.as_ref() == Some(&cached) && !self.dirty {
            return Ok(None);
        }
        let ComponentDesc::Text(mut run) = self
            .native
            .world
            .component(self.status)
            .map_err(|e| e.to_string())?
            .desc
            .clone()
        else {
            unreachable!()
        };
        text.clone_into(&mut run.text);
        self.native
            .world
            .set_component(self.status, ComponentDesc::Text(run))
            .map_err(|e| e.to_string())?;
        let layout = self
            .native
            .world
            .ui_layout_with(size.into(), &self.native.router.layout)
            .map_err(|e| e.to_string())?;
        let mut plan = extract_ui_layout(&self.native.world, &self.native.fonts, layout)
            .map_err(|e| e.to_string())?;
        self.native
            .router
            .style_plan(&self.native.world, &mut plan)
            .map_err(|e| e.to_string())?;
        self.cached = Some(cached);
        self.dirty = false;
        if self.plan.as_ref() == Some(&plan) {
            return Ok(None);
        }
        let overlay = UiOverlay::new(plan.clone()).map_err(|e| e.to_string())?;
        self.plan = Some(plan);
        Ok(Some(Arc::new(overlay)))
    }
}
fn promotion_dialog(native: &mut NativeUi) -> Result<ObjectHandle, WorldError> {
    let dialog = native.rect(native.root, [360., 420.], [0., 0.])?;
    native.router.modals.insert(dialog);
    native.text(dialog, "Promote pawn", [340., 38.], [0., 100.])?;
    for (index, name) in ["Queen", "Rook", "Bishop", "Knight"]
        .into_iter()
        .enumerate()
    {
        let y = -42. - f32::from(u16::try_from(index).expect("four choices")) * 42.;
        let object = native.text(dialog, name, [340., 38.], [0., y])?;
        native.control(object, UiControl::choice(dialog, index));
    }
    native.world.set_active(dialog, false)?;
    Ok(dialog)
}

fn status_widget(native: &mut NativeUi) -> Result<ComponentHandle, WorldError> {
    let status_object = native.text(native.root, "", [0., 0.], [0., 0.])?;
    let status = native.text_handle(status_object);
    let rect = native.rect_handle(status_object);
    native.world.set_component(
        rect,
        ComponentDesc::RectTransform(RectTransform {
            anchor_min: Vec2::new(0., 1.),
            anchor_max: Vec2::new(1., 1.),
            pivot: Vec2::new(0., 1.),
            size_delta: Vec2::new(-32., 90.),
            anchored_position: Vec2::new(16., -12.),
        }),
    )?;
    let ComponentDesc::Text(mut run) = native.world.component(status)?.desc.clone() else {
        unreachable!()
    };
    run.font_size = 22.;
    run.horizontal = worldrunner::ui::HorizontalAlign::Left;
    run.vertical = worldrunner::ui::VerticalAlign::Top;
    native
        .world
        .set_component(status, ComponentDesc::Text(run))?;
    Ok(status)
}
fn claim_widget(native: &mut NativeUi) -> Result<ObjectHandle, WorldError> {
    let claim = native.text(native.root, "Claim draw", [180., 32.], [0., 0.])?;
    let rect = native.rect_handle(claim);
    native.world.set_component(
        rect,
        ComponentDesc::RectTransform(RectTransform {
            anchor_min: Vec2::new(0., 1.),
            anchor_max: Vec2::new(0., 1.),
            anchored_position: Vec2::new(106., -112.),
            size_delta: Vec2::new(180., 32.),
            ..RectTransform::default()
        }),
    )?;
    native.control(claim, UiControl::button());
    native.world.set_active(claim, false)?;
    Ok(claim)
}
