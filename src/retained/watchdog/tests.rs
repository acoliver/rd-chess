use super::{HARD_LIMIT, ProgressWatchdog, STALL_WINDOW};
use std::time::Duration;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
struct FakeProgress {
    accepted_generation: u64,
    preparations_in_flight: usize,
    submitted_frames: u64,
    capture_state: u8,
}

#[derive(Default)]
struct FakeSource {
    now: Duration,
    state: FakeProgress,
}
impl FakeSource {
    fn advance(&mut self, elapsed: Duration) {
        self.now += elapsed;
    }
}

#[test]
fn slow_steady_progress_succeeds_past_the_old_total_deadline() {
    let mut source = FakeSource::default();
    let mut watchdog = ProgressWatchdog::new(source.now, source.state, "capture completion");
    for _ in 0..6 {
        source.advance(Duration::from_secs(50));
        source.state.submitted_frames += 1;
        watchdog.observe(source.now, source.state).unwrap();
    }
    source.state.capture_state = 2;
    watchdog.observe(source.now, source.state).unwrap();
    assert_eq!(source.now, Duration::from_mins(5));
}

#[test]
fn unchanged_polling_fails_at_stall_window_with_signal_and_elapsed_time() {
    let mut source = FakeSource::default();
    let mut watchdog = ProgressWatchdog::new(source.now, source.state, "capture completion");
    source.advance(STALL_WINDOW.checked_sub(Duration::from_secs(1)).unwrap());
    watchdog.observe(source.now, source.state).unwrap();
    source.advance(Duration::from_secs(1));
    let error = watchdog.observe(source.now, source.state).unwrap_err();
    assert!(error.contains("capture completion"), "{error}");
    assert!(error.contains("accepted_generation"), "{error}");
    assert!(error.contains("submitted_frames"), "{error}");
    assert!(error.contains("180s"), "{error}");
}

#[test]
fn endless_progress_still_hits_the_hard_total_bound() {
    let mut source = FakeSource::default();
    let mut watchdog = ProgressWatchdog::new(source.now, source.state, "computer reply");
    while source.now < HARD_LIMIT.checked_sub(Duration::from_secs(100)).unwrap() {
        source.advance(Duration::from_secs(100));
        source.state.submitted_frames += 1;
        watchdog.observe(source.now, source.state).unwrap();
    }
    source.advance(Duration::from_secs(100));
    source.state.submitted_frames += 1;
    let error = watchdog.observe(source.now, source.state).unwrap_err();
    assert!(error.contains("hard limit"), "{error}");
    assert!(error.contains("computer reply"), "{error}");
    assert!(error.contains("1200s"), "{error}");
}

#[test]
fn each_observable_signal_resets_the_stall_clock() {
    let mut source = FakeSource::default();
    let mut watchdog = ProgressWatchdog::new(source.now, source.state, "capture completion");
    for signal in 0..6 {
        source.advance(STALL_WINDOW.checked_sub(Duration::from_secs(1)).unwrap());
        match signal {
            0 => source.state.accepted_generation += 1,
            1 => source.state.preparations_in_flight += 1,
            2 => source.state.preparations_in_flight -= 1,
            3 => source.state.submitted_frames += 1,
            4 => source.state.capture_state = 1,
            _ => source.state.capture_state = 2,
        }
        watchdog.observe(source.now, source.state).unwrap();
    }
    source.advance(Duration::from_secs(100));
    watchdog.observe(source.now, source.state).unwrap();
}

#[test]
fn progress_does_not_reset_the_hard_limit_and_stalls_measure_from_last_change() {
    let mut source = FakeSource::default();
    let mut watchdog = ProgressWatchdog::new(source.now, source.state, "preparation");
    source.advance(Duration::from_secs(50));
    source.state.preparations_in_flight = 1;
    watchdog.observe(source.now, source.state).unwrap();
    source.advance(STALL_WINDOW);
    let error = watchdog.observe(source.now, source.state).unwrap_err();
    assert!(error.contains("180s"), "{error}");
    assert!(error.contains("230s"), "{error}");
    assert!(error.contains("preparation"), "{error}");
}
