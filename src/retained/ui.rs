use glam::Vec2;
use std::sync::Arc;
use worldrunner::{
    scene::gpu::simulation::UiOverlay,
    simulation::{
        ComponentDesc, ComponentHandle, ObjectDesc, ObjectHandle, ParentSpace, World, WorldError,
        WorldSettings,
    },
    ui::{
        Canvas, FontBook, HorizontalAlign, RectTransform, TextRun, UiAction, UiControl, UiKey,
        UiLimits, UiResponse, UiRouter, VerticalAlign, plan::ui::extract_ui,
    },
};

pub(super) struct GameUi {
    world: World,
    fonts: FontBook,
    status: ComponentHandle,
    dialog: ObjectHandle,
    router: UiRouter,
    cached: Option<(String, bool, [u32; 2])>,
}

impl GameUi {
    pub fn new() -> Result<Self, WorldError> {
        let mut world = World::new(WorldSettings::default())?;
        let mut fonts = FontBook::new(UiLimits::default())?;
        let font =
            fonts.register(include_bytes!("../../assets/fonts/SpaceMono-Regular.ttf").to_vec())?;
        let root = rect(&mut world, None, RectTransform::default())?;
        world.add_component(root, ComponentDesc::Canvas(Canvas::default()))?;
        let status_object = rect(
            &mut world,
            Some(root),
            RectTransform {
                anchor_min: Vec2::new(0., 1.),
                anchor_max: Vec2::new(1., 1.),
                pivot: Vec2::new(0., 1.),
                size_delta: Vec2::new(-32., 90.),
                anchored_position: Vec2::new(16., -12.),
            },
        )?;
        let status = world.add_component(
            status_object,
            ComponentDesc::Text(TextRun {
                font_size: 22.,
                ..TextRun::new(font)
            }),
        )?;
        let dialog = rect(
            &mut world,
            Some(root),
            RectTransform {
                size_delta: Vec2::new(360., 420.),
                ..RectTransform::default()
            },
        )?;
        let mut router = UiRouter::default();
        router.modals.insert(dialog);
        label(&mut world, dialog, font, "Promote pawn", 100.)?;
        for (index, name) in ["Queen", "Rook", "Bishop", "Knight"]
            .into_iter()
            .enumerate()
        {
            let y = -42. - f32::from(u16::try_from(index).expect("four choices")) * 42.;
            let choice = label(&mut world, dialog, font, name, y)?;
            router
                .controls
                .insert(choice, UiControl::choice(dialog, index));
        }
        world.set_active(dialog, false)?;
        Ok(Self {
            world,
            fonts,
            status,
            dialog,
            router,
            cached: None,
        })
    }

    pub fn show_dialog(&mut self, active: bool) -> Result<(), WorldError> {
        self.world.set_active(self.dialog, active)?;
        if !active {
            self.pointer(worldrunner::simulation::PointerEvent::Cancel, [1, 1])?;
        }
        Ok(())
    }

    pub fn pointer(
        &mut self,
        event: worldrunner::simulation::PointerEvent,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        let layout = self.world.ui_layout(viewport.into())?;
        self.router.pointer(&self.world, &layout, event)
    }

    pub fn key(
        &mut self,
        key: UiKey,
        pressed: bool,
        repeat: bool,
        viewport: [u32; 2],
    ) -> Result<UiResponse, WorldError> {
        let layout = self.world.ui_layout(viewport.into())?;
        self.router.key(&self.world, &layout, key, pressed, repeat)
    }

    pub fn overlay(
        &mut self,
        text: &str,
        promotion: bool,
        size: [u32; 2],
    ) -> Result<Option<Arc<UiOverlay>>, String> {
        let state = (text.to_owned(), promotion, size);
        if self.cached.as_ref() == Some(&state) {
            return Ok(None);
        }
        let ComponentDesc::Text(mut run) = self
            .world
            .component(self.status)
            .map_err(|e| e.to_string())?
            .desc
            .clone()
        else {
            unreachable!()
        };
        text.clone_into(&mut run.text);
        self.world
            .set_component(self.status, ComponentDesc::Text(run))
            .map_err(|e| e.to_string())?;
        let plan = extract_ui(&self.world, &self.fonts, size).map_err(|e| e.to_string())?;
        let overlay = UiOverlay::new(plan).map_err(|e| e.to_string())?;
        self.cached = Some(state);
        Ok(Some(Arc::new(overlay)))
    }
}

pub(super) fn choice(actions: &[UiAction]) -> Option<usize> {
    actions.iter().find_map(|action| match *action {
        UiAction::ChoiceSelected { index, .. } => Some(index),
        UiAction::Activated { .. } => None,
    })
}

fn rect(
    world: &mut World,
    parent: Option<ObjectHandle>,
    transform: RectTransform,
) -> Result<ObjectHandle, WorldError> {
    let object = world.create_object(ObjectDesc::default())?;
    world.add_component(object, ComponentDesc::RectTransform(transform))?;
    world.set_parent(object, parent, ParentSpace::KeepLocal)?;
    Ok(object)
}

fn label(
    world: &mut World,
    parent: ObjectHandle,
    font: worldrunner::ui::FontId,
    text: &str,
    y: f32,
) -> Result<ObjectHandle, WorldError> {
    let object = rect(
        world,
        Some(parent),
        RectTransform {
            size_delta: Vec2::new(340., 38.),
            anchored_position: Vec2::new(0., y),
            ..RectTransform::default()
        },
    )?;
    world.add_component(
        object,
        ComponentDesc::Text(TextRun {
            text: text.into(),
            font_size: 24.,
            horizontal: HorizontalAlign::Center,
            vertical: VerticalAlign::Middle,
            ..TextRun::new(font)
        }),
    )?;
    Ok(object)
}
