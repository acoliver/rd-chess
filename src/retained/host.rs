use super::{CaptureInbox, ProgressPump, TransactionQueue};
use crate::CameraPreset;
use std::time::Duration;
use worldrunner::scene::{
    gpu::offscreen::RgbaImage,
    plan::RenderScene,
    viewer::{
        ActivityReason, CapturePoolPolicy, FrameProfile, OrbitLimits, PresentMode,
        PresentationBudgetPolicy, PresentationCadence, RenderClock, RetainedCaptureOutcome,
        ViewerError, ViewerOptions, WindowSizeUnit, WindowlessRetained,
    },
};

pub fn viewer_options(scene: &RenderScene, preset: CameraPreset, size: [u32; 2]) -> ViewerOptions {
    ViewerOptions {
        title: "Chess3D Nocturne / Ivory".into(),
        size,
        pose: preset.pose(),
        camera: *scene
            .effects()
            .expect("chess supplies studio lighting")
            .camera(),
        frame_profile: FrameProfile::Legacy,
        frames_in_flight: 2,
        orbit: OrbitLimits {
            min_distance_millimeters: 2_000.0,
            max_distance_millimeters: 40_000.0,
            min_elevation_radians: 0.1,
            max_elevation_radians: 1.47,
        },
        shadows: Some(crate::shadow_settings()),
        camera_convention: worldrunner::scene::camera::ViewConvention::RIGHT_HANDED,
        activity_reason: ActivityReason::default(),
        presentation_budget: PresentationBudgetPolicy::default(),
        capture_pool: CapturePoolPolicy::default(),
        window_size_unit: WindowSizeUnit::default(),
        render_clock: RenderClock::Fixed(Duration::ZERO),
        gpu_timing: false,
        present_mode: PresentMode::Fifo,
        desired_maximum_frame_latency: 2,
        adapter_callback: None,
        cadence: PresentationCadence::default(),
        ssr: None,
    }
}

/// A bounded test-control pump over the engine's windowless session, never a window callback.
pub fn capture_frame(
    run: &mut WindowlessRetained<'_>,
    queue: &TransactionQueue,
    inbox: &CaptureInbox,
) -> Result<RgbaImage, ViewerError> {
    let mut pump = ProgressPump::new(run, "retained capture preparation/completion");
    let mut stable = 0;
    while stable < 2 {
        let generation = run.host().accepted_generation();
        pump.step(run)?;
        if queue.borrow().is_empty()
            && run.host().preparations_in_flight() == 0
            && run.host().accepted_generation() == generation
        {
            stable += 1;
        } else {
            stable = 0;
        }
    }
    let generation = run.host().accepted_generation();
    let id = run.host_mut().request_capture(generation)?;
    pump.capture_accepted()?;
    loop {
        pump.capture_step(run, inbox, id)?;
        if let Some(outcome) = inbox.borrow_mut().remove(&id) {
            return match outcome {
                RetainedCaptureOutcome::Completed { image, .. } => Ok(image),
                other => Err(ViewerError::Driver(format!(
                    "unexpected chess capture outcome: {other:?}"
                ))),
            };
        }
    }
}
