//! Required-device acceptance through the public game and engine facade APIs.
use chess3d::{Position, render_scene};
use glam::Vec3;
use worldrunner::scene::gpu;

fn capture(context: &gpu::device::GpuContext, position: &Position) -> Vec<u8> {
    let scene = render_scene(position).unwrap();
    let view = gpu::SceneView::new(
        gpu::CameraPose {
            eye_millimeters: [8_200, -10_500, 10_000],
            target_millimeters: [0, 0, 400],
        },
        *scene.effects().camera(),
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

#[test]
fn required_gpu_repeats_saved_initial_exactly_and_renders_live_position_and_seed() {
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
}
