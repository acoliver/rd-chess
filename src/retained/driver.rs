use crate::CameraPreset;
use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    rc::Rc,
    time::{Duration, Instant},
};
use worldrunner::scene::{
    gpu::offscreen::RgbaImage,
    plan::RenderScene,
    viewer::{
        ActivityReason, CapturePoolPolicy, FrameProfile, OrbitLimits, PresentMode,
        PresentationBudgetPolicy, PresentationCadence, RenderClock, RetainedCaptureOutcome,
        RetainedDriver, RetainedEvent, RetainedEventHost, RetainedHost, RetainedTick,
        RetainedUpdate, SceneObjectEdit, ViewerError, ViewerOptions, WindowSizeUnit,
        WindowlessRetained,
    },
};

pub type TransactionQueue = Rc<RefCell<VecDeque<Vec<SceneObjectEdit>>>>;
pub type CaptureInbox = Rc<RefCell<BTreeMap<u64, RetainedCaptureOutcome>>>;

#[derive(Default)]
pub struct BoardDriver {
    board: Option<super::RetainedBoard>,
    pending: TransactionQueue,
    captures: CaptureInbox,
}

impl BoardDriver {
    pub fn with_board(board: super::RetainedBoard) -> Self {
        Self {
            board: Some(board),
            ..Self::default()
        }
    }

    pub fn board(&self) -> Option<&super::RetainedBoard> {
        self.board.as_ref()
    }

    pub fn play_uci(&mut self, uci: &str) -> Result<(), String> {
        let board = self
            .board
            .as_mut()
            .ok_or("editor scenes have no legal game")?;
        let edits = board
            .play_uci(uci)
            .map_err(|error| format!("invalid UCI move '{uci}': {error}"))?;
        self.pending.borrow_mut().push_back(edits);
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        let board = self
            .board
            .as_mut()
            .ok_or("editor scenes have no legal game")?;
        let edits = board.undo().map_err(|error| error.to_string())?;
        self.pending.borrow_mut().push_back(edits);
        Ok(())
    }

    pub fn queue(&self) -> TransactionQueue {
        self.pending.clone()
    }
    pub fn captures(&self) -> CaptureInbox {
        self.captures.clone()
    }
}

impl RetainedDriver for BoardDriver {
    fn update(&mut self, host: &mut RetainedHost, _: &RetainedTick) -> Result<(), String> {
        let mut pending = self.pending.borrow_mut();
        if let Some(edits) = pending.front() {
            match host.apply(RetainedUpdate::Objects(edits.clone())) {
                Ok(()) => {
                    pending.pop_front();
                }
                Err(ViewerError::BackPressure { .. }) => (),
                Err(error) => return Err(error.to_string()),
            }
        }
        Ok(())
    }

    fn event(
        &mut self,
        _: &mut RetainedEventHost<'_>,
        event: &RetainedEvent,
    ) -> Result<(), String> {
        if let RetainedEvent::Capture { id, outcome, .. } = event {
            self.captures.borrow_mut().insert(*id, outcome.clone());
        }
        Ok(())
    }
}

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
    let deadline = Instant::now() + Duration::from_mins(1);
    while !queue.borrow().is_empty() {
        progress(run, deadline)?;
    }
    let generation = run.host().accepted_generation();
    let id = run.host_mut().request_capture(generation)?;
    loop {
        progress(run, deadline)?;
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

fn progress(run: &mut WindowlessRetained<'_>, deadline: Instant) -> Result<(), ViewerError> {
    if Instant::now() >= deadline {
        return Err(ViewerError::Driver(
            "chess retained capture timed out".into(),
        ));
    }
    run.frame()?;
    run.poll()?;
    std::thread::yield_now();
    Ok(())
}
