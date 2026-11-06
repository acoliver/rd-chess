pub(super) fn control(
    native: &mut super::widgets::NativeUi,
    parent: worldrunner::simulation::ObjectHandle,
) -> Result<
    (
        worldrunner::simulation::ObjectHandle,
        worldrunner::simulation::ComponentHandle,
        worldrunner::simulation::ComponentHandle,
    ),
    WorldError,
> {
    use worldrunner::{
        simulation::ComponentDesc,
        ui::{CanvasRenderer, UiControl, UiStyle},
    };
    let object = native.rect(parent, [1., 1.], [0., 0.])?;
    native.control(object, UiControl::button());
    native.router.styles.insert(
        object,
        UiStyle {
            normal: [0.; 4],
            focused: [1., 0.72, 0.20, 1.],
            selected: [0.; 4],
            hovered: [0.; 4],
            pressed: [0.; 4],
            disabled: [0.; 4],
        },
    );
    let graphic = native.world.add_component(
        object,
        ComponentDesc::CanvasRenderer(CanvasRenderer {
            flags: worldrunner::ui::CanvasRendererFlags {
                cull: true,
                ..worldrunner::ui::CanvasRendererFlags::default()
            },
            geometry: Some(border(Vec2::ONE)?),
            ..CanvasRenderer::default()
        }),
    )?;
    Ok((object, native.rect_handle(object), graphic))
}

use glam::Vec2;
use std::sync::Arc;
use worldrunner::{
    simulation::WorldError,
    ui::{UiGeometry, UiVertex},
};

pub(super) fn border(size: Vec2) -> Result<Arc<UiGeometry>, WorldError> {
    let thickness = 3_f32.min(size.min_element() / 8.);
    let strips = [
        (Vec2::ZERO, Vec2::new(size.x, thickness)),
        (Vec2::new(0., size.y - thickness), size),
        (
            Vec2::new(0., thickness),
            Vec2::new(thickness, size.y - thickness),
        ),
        (
            Vec2::new(size.x - thickness, thickness),
            Vec2::new(size.x, size.y - thickness),
        ),
    ];
    let mut vertices = Vec::new();
    let mut indices = Vec::new();
    for (low, high) in strips {
        let base = u32::try_from(vertices.len()).expect("four border quads");
        for point in [
            low,
            Vec2::new(high.x, low.y),
            high,
            Vec2::new(low.x, high.y),
        ] {
            vertices.push(UiVertex {
                position: [point.x, point.y, 0.],
                uv: [0., 0.],
                color: [1.; 4],
            });
        }
        indices.extend([base, base + 1, base + 2, base + 2, base + 3, base]);
    }
    Ok(Arc::new(UiGeometry::new(vertices, indices)?))
}
