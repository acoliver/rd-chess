use super::{CaptureInbox, watchdog::ProgressWatchdog};
use std::time::{Duration, Instant};
use worldrunner::scene::viewer::{ViewerError, WindowlessRetained};

#[derive(Clone, Copy, Debug, PartialEq)]
enum CaptureState {
    Preparing,
    Accepted,
    Completed,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Progress {
    accepted_generation: u64,
    preparations_in_flight: usize,
    submitted_frames: u64,
    capture_state: CaptureState,
}
impl Progress {
    fn sample(run: &WindowlessRetained<'_>) -> Self {
        Self {
            accepted_generation: run.host().accepted_generation(),
            preparations_in_flight: run.host().preparations_in_flight(),
            submitted_frames: 0,
            capture_state: CaptureState::Preparing,
        }
    }
}

/// Pumps observable session progress with a 180-second stall window and 1,200-second hard bound.
/// Bounds are checked between engine calls, not by interrupting synchronous GPU work.
pub struct ProgressPump {
    clock: Instant,
    state: Progress,
    watchdog: ProgressWatchdog<Progress>,
}
impl ProgressPump {
    pub fn new(run: &WindowlessRetained<'_>, waiting: &'static str) -> Self {
        let state = Progress::sample(run);
        Self {
            clock: Instant::now(),
            state,
            watchdog: ProgressWatchdog::new(Duration::ZERO, state, waiting),
        }
    }

    pub fn step(&mut self, run: &mut WindowlessRetained<'_>) -> Result<(), ViewerError> {
        self.step_with(run, || false)
    }

    pub(super) fn capture_step(
        &mut self,
        run: &mut WindowlessRetained<'_>,
        inbox: &CaptureInbox,
        id: u64,
    ) -> Result<(), ViewerError> {
        self.step_with(run, || inbox.borrow().contains_key(&id))
    }

    fn step_with(
        &mut self,
        run: &mut WindowlessRetained<'_>,
        completed: impl Fn() -> bool,
    ) -> Result<(), ViewerError> {
        self.observe()?;
        self.state.submitted_frames += u64::from(run.frame()?);
        self.sample(run, completed())?;
        run.poll()?;
        self.sample(run, completed())?;
        std::thread::yield_now();
        Ok(())
    }

    pub(super) fn capture_accepted(&mut self) -> Result<(), ViewerError> {
        self.state.capture_state = CaptureState::Accepted;
        self.observe()
    }

    fn sample(&mut self, run: &WindowlessRetained<'_>, completed: bool) -> Result<(), ViewerError> {
        self.state.accepted_generation = run.host().accepted_generation();
        self.state.preparations_in_flight = run.host().preparations_in_flight();
        if completed {
            self.state.capture_state = CaptureState::Completed;
        }
        self.observe()
    }

    fn observe(&mut self) -> Result<(), ViewerError> {
        self.watchdog
            .observe(self.clock.elapsed(), self.state)
            .map_err(ViewerError::Driver)
    }
}
