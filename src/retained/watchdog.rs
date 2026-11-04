use std::{fmt::Debug, time::Duration};

// A single compile on the loaded gate host can exceed the old 60-second total limit.
const STALL_WINDOW: Duration = Duration::from_mins(3);
// Over 30 times the passing 25-36 second GPU suites, independent of progress.
const HARD_LIMIT: Duration = Duration::from_mins(20);

pub(crate) struct ProgressWatchdog<T> {
    started: Duration,
    changed: Duration,
    state: T,
    waiting: &'static str,
}

impl<T: PartialEq + Debug> ProgressWatchdog<T> {
    pub(crate) fn new(now: Duration, state: T, waiting: &'static str) -> Self {
        Self {
            started: now,
            changed: now,
            state,
            waiting,
        }
    }

    pub(crate) fn observe(&mut self, now: Duration, state: T) -> Result<(), String> {
        let elapsed = now.checked_sub(self.started).expect("monotonic clock");
        if elapsed >= HARD_LIMIT {
            return Err(format!(
                "chess {} hit hard limit after {elapsed:?}; last progress: {:?}",
                self.waiting, self.state
            ));
        }
        if self.state != state {
            self.changed = now;
            self.state = state;
        }
        let stalled = now.checked_sub(self.changed).expect("monotonic clock");
        if stalled >= STALL_WINDOW {
            return Err(format!(
                "chess {} stalled for {stalled:?} ({elapsed:?} total); unchanged progress: {:?}",
                self.waiting, self.state
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
