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

pub(super) struct NativeUi {
    pub world: World,
    pub fonts: FontBook,
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
        Ok(Self {
            world,
            fonts,
            font,
            root,
            router: UiRouter::default(),
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
                disabled: [0.35, 0.35, 0.35, 1.],
                ..UiStyle::default()
            },
        );
    }
    fn background(&mut self, object: ObjectHandle, size: [f32; 2]) -> Result<(), WorldError> {
        let [x, y] = size.map(|v| v * 0.5);
        let vertices = [
            ([-x, -y, 0.], [0., 0.]),
            ([x, -y, 0.], [1., 0.]),
            ([x, y, 0.], [1., 1.]),
            ([-x, y, 0.], [0., 1.]),
        ]
        .map(|(position, uv)| UiVertex {
            position,
            uv,
            color: [0.025, 0.035, 0.055, 0.96],
        });
        let geometry = UiGeometry::new(vertices.to_vec(), vec![0, 1, 2, 2, 3, 0])?;
        self.world.add_component(
            object,
            ComponentDesc::CanvasRenderer(CanvasRenderer {
                geometry: Some(Arc::new(geometry)),
                ..CanvasRenderer::default()
            }),
        )?;
        Ok(())
    }
}
