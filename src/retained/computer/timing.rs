use super::{BoardDriver, SEARCH_BUDGET};
use std::{sync::atomic::Ordering, time::Duration};
use worldrunner::scene::viewer::{RetainedHost, TimerError, TimerHandle};

const THINKING_DELAY: Duration = Duration::from_millis(300);

pub(super) struct SearchTimers {
    cap: TimerHandle,
    thinking: TimerHandle,
    pub visible: bool,
}
impl SearchTimers {
    pub fn new(host: &mut RetainedHost) -> Result<Self, TimerError> {
        let now = host.timer_now();
        let cap = host.timer_at(now + SEARCH_BUDGET)?;
        match host.timer_at(now + THINKING_DELAY) {
            Ok(thinking) => Ok(Self {
                cap,
                thinking,
                visible: false,
            }),
            Err(error) => {
                host.cancel_timer(&cap);
                Err(error)
            }
        }
    }
    pub fn cancel(self, mut cancel: impl FnMut(&TimerHandle)) {
        cancel(&self.cap);
        cancel(&self.thinking);
    }
}
impl BoardDriver {
    pub(in crate::retained) fn computer_changed(&mut self) {
        if let Some(timers) = self.computer.as_mut().and_then(super::Computer::changed) {
            self.retire_search_timers(timers);
        }
    }
    pub(super) fn retire_search_timers(&mut self, timers: SearchTimers) {
        // Public game actions have no host borrow. Flush at the current event's
        // end (before another due timer), or the next retained update boundary.
        timers.cancel(|handle| self.cancelled_timers.push(handle.clone()));
    }
    pub(in crate::retained) fn flush_computer_timers(
        &mut self,
        mut cancel: impl FnMut(&TimerHandle),
    ) {
        for handle in self.cancelled_timers.drain(..) {
            cancel(&handle);
        }
    }
    pub(in crate::retained) fn computer_timer(&mut self, handle: &TimerHandle) {
        let Some(request) = self.computer.as_mut().and_then(|c| c.pending.as_mut()) else {
            return;
        };
        if *handle == request.timers.cap {
            // Keep the ticket live: cancellation would discard its completed
            // iteration instead of delivering it through WorkReady.
            request.stop.store(true, Ordering::Relaxed);
        } else if *handle == request.timers.thinking {
            request.timers.visible = true;
        }
    }
}
