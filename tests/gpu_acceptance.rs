//! Required-device acceptance through the public game and engine facade APIs.
use chess3d::{Position, render_scene};
use glam::Vec3;
use worldrunner::scene::gpu;

#[test]
fn required_gpu_retained_renderer_survives_target_recreation_and_device_lifetimes() {
    use gpu::offscreen::OffscreenTarget;
    use gpu::shadows::{DirectionalShadowSettings, ShadowedScene};

    let position = Position::initial(342);
    let original = {
        let context = required_context();
        let scene = render_scene(&position).unwrap();
        let settings = DirectionalShadowSettings {
            focus_meters: Vec3::ZERO,
            half_extent_meters: 8.0,
            depth_half_extent_meters: 16.0,
            resolution: 512,
            depth_bias: 0.0008,
            normal_bias_meters: 0.012,
        };
        let mut renderer = ShadowedScene::new(&context, &scene, settings).unwrap();
        let mut original = None;
        for (width, height) in [(257, 193), (321, 201), (257, 193)] {
            let view = gpu::SceneView::new(
                gpu::CameraPose {
                    eye_millimeters: [8_200, -10_500, 10_000],
                    target_millimeters: [0, 0, 400],
                },
                *scene
                    .effects()
                    .expect("chess supplies studio lighting")
                    .camera(),
                gpu::Viewport { width, height },
            )
            .unwrap();
            let mut target = OffscreenTarget::new(context.device(), view.viewport()).unwrap();
            let pixels = retained_capture(&context, &mut renderer, &view, &mut target);
            let row_bytes = usize::try_from(width).unwrap() * 4;
            assert_eq!(pixels.len(), row_bytes * usize::try_from(height).unwrap());
            assert!(pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] > 150));
            assert!(pixels.chunks_exact(4).any(|pixel| pixel[0] < 80));
            assert_eq!(
                pixels,
                retained_capture(&context, &mut renderer, &view, &mut target),
                "reused odd-width target must read back identical tightly packed rows"
            );
            if width == 257 {
                if let Some(expected) = &original {
                    assert_eq!(
                        &pixels, expected,
                        "restoring viewport must restore exact pixels"
                    );
                } else {
                    original = Some(pixels);
                }
            }
        }
        original.unwrap()
    };
    let context = required_context();
    assert_eq!(
        original,
        capture(&context, &position),
        "new device and resources must not inherit previous renderer state"
    );
}

fn capture(context: &gpu::device::GpuContext, position: &Position) -> Vec<u8> {
    let scene = render_scene(position).unwrap();
    let view = gpu::SceneView::new(
        gpu::CameraPose {
            eye_millimeters: [8_200, -10_500, 10_000],
            target_millimeters: [0, 0, 400],
        },
        *scene
            .effects()
            .expect("chess supplies studio lighting")
            .camera(),
        gpu::Viewport {
            width: 257,
            height: 193,
        },
    )
    .unwrap();
    let image = gpu::shadows::capture_scene_shadowed(
        context,
        &scene,
        &view,
        gpu::shadows::DirectionalShadowSettings {
            focus_meters: Vec3::ZERO,
            half_extent_meters: 8.0,
            depth_half_extent_meters: 16.0,
            resolution: 512,
            depth_bias: 0.0008,
            normal_bias_meters: 0.012,
        },
    )
    .unwrap();
    assert_eq!(image.pixels.len(), 257 * 193 * 4);
    assert!(image.pixels.chunks_exact(4).all(|p| p[3] == 255));
    assert!(image.pixels.chunks_exact(4).any(|p| p[0] > 150));
    assert!(image.pixels.chunks_exact(4).any(|p| p[0] < 80));
    image.pixels
}

fn required_context() -> gpu::device::GpuContext {
    let context = pollster::block_on(gpu::device::GpuContext::request_with_features(
        gpu::wgpu::Features::empty(),
    ))
    .expect(
        "Chess3D acceptance requires a real GPU device; missing adapters are failures, not skips",
    );
    println!(
        "Chess3D acceptance adapter: {:?}",
        context.adapter().get_info()
    );
    context
}

#[test]
fn required_gpu_repeats_saved_initial_exactly_and_renders_live_position_and_seed() {
    let context = required_context();
    let initial = Position::initial(342);
    let pixels = capture(&context, &initial);
    assert_eq!(pixels, capture(&context, &initial), "same-device repeat");
    let directory = tempfile::tempdir().unwrap();
    let save = directory.path().join("initial.json");
    initial.save_atomic(&save).unwrap();
    let loaded = Position::load(&save).unwrap();
    assert_eq!(initial, loaded);
    assert_eq!(pixels, capture(&context, &loaded), "initial save/load");
    let study = Position::load(
        &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/positions/study.json"),
    )
    .unwrap();
    assert_ne!(pixels, capture(&context, &study), "live study position");
    assert_ne!(
        pixels,
        capture(&context, &Position::initial(342 + (1 << 48))),
        "high seed bits must affect rendered marble"
    );
    piece_shadow_follows_light(&context);
}

fn effects_with_sun_and_cascades(
    source: &worldrunner::scene::plan::SceneEffects,
    reverse: bool,
    extents: Vec<u64>,
) -> worldrunner::scene::plan::SceneEffects {
    use worldrunner::scene::plan::{SceneEffects, SceneEffectsParts, SceneShadowCascades};
    let mut sun = *source.sun();
    if reverse {
        let [x, y, z] = sun.direction.components_micro();
        sun.direction = worldrunner::scene::plan::SceneDirection::new([-x, -y, z]).unwrap();
    }
    SceneEffects::new(SceneEffectsParts {
        water: source.water().copied(),
        sky: *source.sky(),
        sun,
        ambient: *source.ambient(),
        shadows: SceneShadowCascades::new(extents, 0).unwrap(),
        ssao: source.ssao().copied(),
        camera: *source.camera(),
        grading: *source.grading(),
        post: source.post().to_vec(),
        anti_alias: *source.anti_alias(),
    })
    .unwrap()
}

fn piece_shadow_follows_light(context: &gpu::device::GpuContext) {
    use chess3d::{CameraPreset, PieceKind, Side, Square};
    use gpu::shadows::DirectionalShadowSettings;

    let mut position = Position::initial(342);
    position
        .pieces
        .retain(|piece| piece.kind == PieceKind::Queen && piece.side == Side::Ivory);
    position.pieces[0].square = Square::new(3, 3).unwrap();
    let scene = render_scene(&position).unwrap();
    let view = gpu::SceneView::new(
        CameraPreset::Overhead.pose(),
        *scene
            .effects()
            .expect("chess supplies studio lighting")
            .camera(),
        gpu::Viewport {
            width: 513,
            height: 513,
        },
    )
    .unwrap();
    let settings = DirectionalShadowSettings {
        focus_meters: Vec3::ZERO,
        half_extent_meters: 8.0,
        depth_half_extent_meters: 16.0,
        resolution: 1024,
        depth_bias: 0.0008,
        normal_bias_meters: 0.012,
    };
    assert_light_updates(context, &scene, &view, settings);
    position.pieces.clear();
    let empty = render_scene(&position).unwrap();
    let shadowed = gpu::shadows::capture_scene_shadowed(context, &empty, &view, settings).unwrap();
    let unshadowed = gpu::shadows::capture_scene_shadowed(
        context,
        &empty,
        &view,
        DirectionalShadowSettings {
            depth_bias: 0.9,
            ..settings
        },
    )
    .unwrap();
    assert_eq!(
        shadow_receiver_counts(&view, &shadowed.pixels, &unshadowed.pixels),
        [0, 0],
        "removing the queen must remove its board shadow"
    );
}

fn assert_light_updates(
    context: &gpu::device::GpuContext,
    scene: &worldrunner::scene::plan::RenderScene,
    view: &gpu::SceneView,
    settings: gpu::shadows::DirectionalShadowSettings,
) {
    use gpu::offscreen::OffscreenTarget;
    use gpu::shadows::{DirectionalShadowSettings, ShadowError, ShadowedScene};

    let mut renderer = ShadowedScene::new(context, scene, settings).unwrap();
    let mut target = OffscreenTarget::new(context.device(), view.viewport()).unwrap();
    for reverse in [false, true] {
        let effects =
            effects_with_sun_and_cascades(scene.effects().unwrap(), reverse, vec![48_000]);
        renderer.set_effects(&effects).unwrap();
        renderer.set_settings(context, settings).unwrap();
        let shadowed = retained_capture(context, &mut renderer, view, &mut target);
        assert!(matches!(
            renderer.set_effects(&effects_with_sun_and_cascades(
                scene.effects().unwrap(),
                !reverse,
                vec![12_000, 24_000, 36_000, 48_000, 60_000],
            )),
            Err(ShadowError::UnsupportedCascades)
        ));
        assert_eq!(
            shadowed,
            retained_capture(context, &mut renderer, view, &mut target),
            "rejected cascade/light update must preserve the rendered frame"
        );
        renderer
            .set_settings(
                context,
                DirectionalShadowSettings {
                    depth_bias: 0.9,
                    ..settings
                },
            )
            .unwrap();
        let unshadowed = retained_capture(context, &mut renderer, view, &mut target);
        let dark_receivers = shadow_receiver_counts(view, &shadowed, &unshadowed);
        println!("Chess queen shadow receivers (reverse={reverse}): {dark_receivers:?}");
        let shadow_side = usize::from(reverse);
        assert!(
            dark_receivers[shadow_side] >= 3,
            "queen must cast a shadow away from the sun: {dark_receivers:?}"
        );
        assert_eq!(
            dark_receivers[1 - shadow_side],
            0,
            "opposite board receivers must remain lit"
        );
    }
}

fn retained_capture(
    context: &gpu::device::GpuContext,
    renderer: &mut gpu::shadows::ShadowedScene,
    view: &gpu::SceneView,
    target: &mut gpu::offscreen::OffscreenTarget,
) -> Vec<u8> {
    renderer
        .render(
            context,
            view,
            &target.color_attachment(),
            &target.depth_attachment(),
        )
        .unwrap();
    target.read_rgba8(context).unwrap().pixels
}

fn shadow_receiver_counts(view: &gpu::SceneView, shadowed: &[u8], unshadowed: &[u8]) -> [usize; 2] {
    let mut dark_receivers = [0; 2];
    // Sample board receivers beyond the queen's silhouette on both sides.
    // A large comparison bias removes occlusion without changing the scene/light.
    for (side, sign) in [1.0, -1.0].into_iter().enumerate() {
        for x in 6_u16..15 {
            for y in 3_u16..12 {
                let point = Vec3::new(
                    -0.5 + sign * f32::from(x) * 0.1,
                    -0.5 + sign * f32::from(y) * 0.1,
                    0.05,
                );
                let index = receiver_pixel(view, point);
                let delta = i16::from(unshadowed[index]) - i16::from(shadowed[index]);
                dark_receivers[side] += usize::from(delta > 10);
            }
        }
    }
    dark_receivers
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "known visible board receivers project within the bounded viewport"
)]
fn receiver_pixel(view: &gpu::SceneView, point: Vec3) -> usize {
    let ndc = view.view_projection_matrix().project_point3(point);
    assert!(ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0 && (0.0..1.0).contains(&ndc.z));
    let x = ((ndc.x * 0.5 + 0.5) * 513.0) as usize;
    let y = ((0.5 - ndc.y * 0.5) * 513.0) as usize;
    (y * 513 + x) * 4
}
