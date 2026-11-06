#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use glam::Vec2;
use std::sync::Arc;
use worldrunner::{
    simulation::{
        ComponentDesc, ObjectDesc, ObjectHandle, ParentSpace, World, WorldError, WorldSettings,
    },
    ui::{
        Canvas, CanvasRenderer, FontBook, FontId, HorizontalAlign, RectTransform, TextRun, UiAlign,
        UiAxis, UiControl, UiGeometry, UiLayoutGroup, UiLimits, UiRouter, UiStyle, UiVertex,
        VerticalAlign,
    },
};

// Opaque panels keep board colours out of browser text and control contrast.
pub(super) const PANEL_ALPHA: f32 = 1.;
const PANEL_COLOR: [f32; 4] = [0.025, 0.035, 0.055, PANEL_ALPHA];

pub(super) struct NativeUi {
    pub world: World,
    pub fonts: Arc<FontBook>,
    pub router: UiRouter,
    pub root: ObjectHandle,
    font: FontId,
}
impl NativeUi {
    pub fn text_handle(&self, object: ObjectHandle) -> worldrunner::simulation::ComponentHandle {
        self.world
            .published_components()
            .find(|c| c.object == object && matches!(c.desc, ComponentDesc::Text(_)))
            .expect("authored text")
            .handle
    }
    pub fn rect_handle(&self, object: ObjectHandle) -> worldrunner::simulation::ComponentHandle {
        self.world
            .published_components()
            .find(|c| c.object == object && matches!(c.desc, ComponentDesc::RectTransform(_)))
            .expect("authored rectangle")
            .handle
    }
    pub fn new() -> Result<Self, WorldError> {
        let mut world = World::new(WorldSettings::default())?;
        let mut fonts = FontBook::new(UiLimits::default())?;
        let font = fonts
            .register(include_bytes!("../../../assets/fonts/SpaceMono-Regular.ttf").to_vec())?;
        let root = world.create_object(ObjectDesc::default())?;
        world.add_component(root, ComponentDesc::RectTransform(RectTransform::default()))?;
        world.add_component(root, ComponentDesc::Canvas(Canvas::default()))?;
        let fonts = Arc::new(fonts);
        Ok(Self {
            world,
            router: UiRouter::with_fonts(fonts.clone()),
            fonts,
            font,
            root,
        })
    }
    pub fn rect(
        &mut self,
        parent: ObjectHandle,
        size: [f32; 2],
        at: [f32; 2],
    ) -> Result<ObjectHandle, WorldError> {
        let object = self.world.create_object(ObjectDesc::default())?;
        self.world.add_component(
            object,
            ComponentDesc::RectTransform(RectTransform {
                size_delta: Vec2::from_array(size),
                anchored_position: Vec2::from_array(at),
                ..RectTransform::default()
            }),
        )?;
        self.world
            .set_parent(object, Some(parent), ParentSpace::KeepLocal)?;
        Ok(object)
    }
    pub fn text(
        &mut self,
        parent: ObjectHandle,
        text: &str,
        size: [f32; 2],
        at: [f32; 2],
    ) -> Result<ObjectHandle, WorldError> {
        let object = self.rect(parent, size, at)?;
        if self.router.modals.contains(&parent) && !text.is_empty() {
            self.router
                .semantics
                .entry(parent)
                .or_insert_with(|| worldrunner::ui::UiSemantics::label(text));
        }
        self.world.add_component(
            object,
            ComponentDesc::Text(TextRun {
                text: text.into(),
                font_size: 18.,
                horizontal: HorizontalAlign::Center,
                vertical: VerticalAlign::Middle,
                ..TextRun::new(self.font)
            }),
        )?;
        Ok(object)
    }
    pub fn panel(&mut self, size: [f32; 2]) -> Result<ObjectHandle, WorldError> {
        let object = self.rect(self.root, size, [0., 0.])?;
        self.background(object, size)?;
        self.router.modals.insert(object);
        self.world.set_active(object, false)?;
        Ok(object)
    }
    pub fn stack(&mut self, object: ObjectHandle, axis: UiAxis, spacing: u16) {
        self.router.layout.groups.insert(
            object,
            UiLayoutGroup::Stack {
                axis,
                spacing,
                padding: [0; 4],
                align: UiAlign::Center,
            },
        );
    }
    pub fn control(&mut self, object: ObjectHandle, control: UiControl) {
        self.router.controls.insert(object, control);
        self.router.styles.insert(
            object,
            UiStyle {
                focused: [1., 0.72, 0.20, 1.],
                selected: [0.35, 0.9, 1., 1.],
                hovered: [0.7, 0.85, 1., 1.],
                pressed: [1., 0.5, 0.1, 1.],
                disabled: [0.55, 0.55, 0.55, 1.],
                ..UiStyle::default()
            },
        );
    }
    pub(super) fn background(
        &mut self,
        object: ObjectHandle,
        size: [f32; 2],
    ) -> Result<(), WorldError> {
        let ComponentDesc::RectTransform(rect) =
            self.world.component(self.rect_handle(object))?.desc
        else {
            unreachable!("background rectangle");
        };
        self.world.add_component(
            object,
            ComponentDesc::CanvasRenderer(CanvasRenderer {
                geometry: Some(panel_geometry(size.into(), rect.pivot)?),
                ..CanvasRenderer::default()
            }),
        )?;
        Ok(())
    }
    pub(super) fn resize_background(
        &mut self,
        rect: &worldrunner::ui::RectLayout,
    ) -> Result<(), WorldError> {
        let component = self
            .world
            .published_components()
            .find(|component| {
                component.object == rect.object
                    && matches!(component.desc, ComponentDesc::CanvasRenderer(_))
            })
            .expect("panel background");
        let handle = component.handle;
        let ComponentDesc::CanvasRenderer(mut renderer) = component.desc.clone() else {
            unreachable!()
        };
        renderer.geometry = Some(panel_geometry(rect.size, rect.pivot)?);
        self.world
            .set_component(handle, ComponentDesc::CanvasRenderer(renderer))
    }
}

fn panel_geometry(size: Vec2, pivot: Vec2) -> Result<Arc<UiGeometry>, WorldError> {
    let min = -size * pivot;
    let max = size * (Vec2::ONE - pivot);
    let vertices = [
        ([min.x, min.y, 0.], [0., 0.]),
        ([max.x, min.y, 0.], [1., 0.]),
        ([max.x, max.y, 0.], [1., 1.]),
        ([min.x, max.y, 0.], [0., 1.]),
    ]
    .map(|(position, uv)| UiVertex {
        position,
        uv,
        color: PANEL_COLOR,
    });
    Ok(Arc::new(UiGeometry::new(
        vertices.to_vec(),
        vec![0, 1, 2, 2, 3, 0],
    )?))
}

impl NativeUi {
    pub fn set_text(
        &mut self,
        handle: worldrunner::simulation::ComponentHandle,
        text: &str,
    ) -> Result<(), WorldError> {
        let ComponentDesc::Text(mut run) = self.world.component(handle)?.desc.clone() else {
            unreachable!("text");
        };
        text.clone_into(&mut run.text);
        self.world.set_component(handle, ComponentDesc::Text(run))
    }
    pub fn left_text(
        &mut self,
        handle: worldrunner::simulation::ComponentHandle,
        size: f32,
    ) -> Result<(), WorldError> {
        let ComponentDesc::Text(mut run) = self.world.component(handle)?.desc.clone() else {
            unreachable!("text");
        };
        run.font_size = size;
        run.horizontal = HorizontalAlign::Left;
        run.vertical = VerticalAlign::Top;
        self.world.set_component(handle, ComponentDesc::Text(run))
    }
    pub fn bottom_rect(
        &mut self,
        object: ObjectHandle,
        inset: [f32; 2],
        size: [f32; 2],
    ) -> Result<(), WorldError> {
        let handle = self.rect_handle(object);
        self.world.set_component(
            handle,
            ComponentDesc::RectTransform(RectTransform {
                anchor_min: Vec2::new(0., 0.),
                anchor_max: Vec2::new(1., 0.),
                pivot: Vec2::new(0.5, 0.),
                anchored_position: Vec2::new(inset[0], 12.),
                size_delta: Vec2::from_array(size) + Vec2::new(inset[1], 0.),
            }),
        )
    }
}
