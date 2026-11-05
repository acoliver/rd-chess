use super::*;
use std::{cell::RefCell, rc::Rc};
use worldrunner::scene::viewer::{
    RetainedDriver, RetainedEvent, RetainedEventHost, RetainedTick, WindowlessRetained,
};

struct Probe {
    driver: Rc<RefCell<BoardDriver>>,
    timers: Rc<RefCell<usize>>,
}
impl RetainedDriver for Probe {
    fn update(&mut self, host: &mut RetainedHost, tick: &RetainedTick) -> Result<(), String> {
        self.driver.borrow_mut().update(host, tick)
    }
    fn event(
        &mut self,
        host: &mut RetainedEventHost<'_>,
        event: &RetainedEvent,
    ) -> Result<(), String> {
        if matches!(event, RetainedEvent::Timer { .. }) {
            *self.timers.borrow_mut() += 1;
        }
        self.driver.borrow_mut().event(host, event)
    }
}
fn probe() -> (Probe, RetainedHost) {
    let (mut driver, host) = setup(INITIAL_FEN, Side::Ivory);
    driver.play_uci("e2e4").unwrap();
    (
        Probe {
            driver: Rc::new(RefCell::new(driver)),
            timers: Rc::default(),
        },
        host,
    )
}
fn slow_reply(driver: &mut BoardDriver, host: &mut RetainedHost) -> Arc<AtomicBool> {
    let chess_move = game(driver).parse_uci("g8f6").unwrap();
    let computer = driver.computer.as_mut().unwrap();
    computer.queue = Some(host.work_queue(1, 1).unwrap());
    let generation = computer.generation;
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let (started, running) = std::sync::mpsc::channel();
    let ticket = computer
        .queue
        .as_ref()
        .unwrap()
        .submit(move |cancel| {
            started.send(()).unwrap();
            while !worker_stop.load(Ordering::Relaxed) && !cancel.is_cancelled() {
                std::thread::yield_now();
            }
            Reply {
                generation,
                chess_move: Some(chess_move),
            }
        })
        .unwrap();
    let mut request = Request::new(ticket, generation, host);
    request.stop = stop.clone();
    computer.pending = Some(request);
    running.recv_timeout(Duration::from_secs(5)).unwrap();
    stop
}
fn poll_reply(run: &mut WindowlessRetained<'_>, driver: &RefCell<BoardDriver>) {
    let clock = Instant::now();
    let mut watchdog = crate::retained::watchdog::ProgressWatchdog::new(
        Duration::ZERO,
        true,
        "timer-controlled computer reply",
    );
    while driver.borrow().computer.as_ref().unwrap().pending.is_some() {
        run.poll().unwrap();
        watchdog
            .observe(
                clock.elapsed(),
                driver.borrow().computer.as_ref().unwrap().pending.is_some(),
            )
            .unwrap();
        std::thread::yield_now();
    }
}

#[test]
fn session_time_expiry_stops_search_and_plays_completed_move() {
    let (mut probe, host) = probe();
    let driver = probe.driver.clone();
    let timers = probe.timers.clone();
    let mut run = host.windowless(&mut probe).unwrap();
    let stop = slow_reply(&mut driver.borrow_mut(), run.host_mut());
    run.advance_time(SEARCH_BUDGET.checked_sub(Duration::from_millis(1)).unwrap())
        .unwrap();
    run.poll().unwrap();
    assert!(!stop.load(Ordering::Relaxed));
    run.advance_time(Duration::from_millis(1)).unwrap();
    run.poll().unwrap();
    assert!(
        stop.load(Ordering::Relaxed),
        "host timer must stop the worker"
    );
    poll_reply(&mut run, &driver);
    assert_eq!(game(&driver.borrow()).repetition_history().len(), 3);
    assert_eq!(
        game(&driver.borrow()).piece_at(Square::new(5, 5).unwrap()),
        Some((Side::Onyx, PieceKind::Knight))
    );
    assert_eq!(*timers.borrow(), 2);
}

#[test]
fn thinking_label_uses_only_session_time_and_disappears_with_reply() {
    let (mut probe, host) = probe();
    let driver = probe.driver.clone();
    let mut run = host.windowless(&mut probe).unwrap();
    let stop = slow_reply(&mut driver.borrow_mut(), run.host_mut());
    assert!(!driver.borrow().status_text().contains("thinking"));
    for _ in 0..3 {
        run.poll().unwrap();
    }
    assert!(!driver.borrow().status_text().contains("thinking"));
    run.advance_time(Duration::from_millis(299)).unwrap();
    run.poll().unwrap();
    assert!(!driver.borrow().status_text().contains("thinking"));
    run.advance_time(Duration::from_millis(1)).unwrap();
    run.poll().unwrap();
    assert_eq!(
        driver.borrow().status_text(),
        "Computer (Level 1, Pip) is thinking"
    );
    stop.store(true, Ordering::Relaxed);
    poll_reply(&mut run, &driver);
    assert!(!driver.borrow().status_text().contains("thinking"));
}

#[test]
fn ready_reply_cancels_even_due_timers_before_their_delivery() {
    let (mut probe, host) = probe();
    let driver = probe.driver.clone();
    let timers = probe.timers.clone();
    let mut run = host.windowless(&mut probe).unwrap();
    let stop = slow_reply(&mut driver.borrow_mut(), run.host_mut());
    stop.store(true, Ordering::Relaxed);
    // Observe the queued completion without consuming it, so WorkReady and both
    // timers are due together on the next session dispatch.
    let computer = driver.borrow();
    let queue = computer.computer.as_ref().unwrap().queue.as_ref().unwrap();
    wait_ready(queue);
    drop(computer);
    run.advance_time(SEARCH_BUDGET).unwrap();
    run.poll().unwrap();
    assert_eq!(game(&driver.borrow()).repetition_history().len(), 3);
    assert_eq!(*timers.borrow(), 0);
    run.advance_time(SEARCH_BUDGET).unwrap();
    run.poll().unwrap();
    assert_eq!(*timers.borrow(), 0);
}

fn wait_ready(queue: &WorkQueue<Reply>) {
    // One worker commits the preceding result before dequeuing this barrier.
    let (started, running) = std::sync::mpsc::channel();
    queue
        .submit(move |_| {
            started.send(()).unwrap();
            Reply {
                generation: u64::MAX,
                chess_move: None,
            }
        })
        .unwrap();
    running.recv_timeout(Duration::from_secs(5)).unwrap();
}

#[test]
fn production_submission_schedules_a_cap_even_while_the_worker_is_busy() {
    let (mut probe, host) = probe();
    let driver = probe.driver.clone();
    let mut run = host.windowless(&mut probe).unwrap();
    let queue = run.host_mut().work_queue(1, 1).unwrap();
    let release = Arc::new(AtomicBool::new(false));
    let worker_release = release.clone();
    let (started, running) = std::sync::mpsc::channel();
    queue
        .submit(move |cancel| {
            started.send(()).unwrap();
            while !worker_release.load(Ordering::Relaxed) && !cancel.is_cancelled() {
                std::thread::yield_now();
            }
            Reply {
                generation: u64::MAX,
                chess_move: None,
            }
        })
        .unwrap();
    running.recv_timeout(Duration::from_secs(5)).unwrap();
    driver.borrow_mut().computer.as_mut().unwrap().queue = Some(queue);
    driver.borrow_mut().advance_computer(run.host_mut());
    let stop = driver
        .borrow()
        .computer
        .as_ref()
        .unwrap()
        .pending
        .as_ref()
        .unwrap()
        .stop
        .clone();
    run.advance_time(SEARCH_BUDGET).unwrap();
    run.poll().unwrap();
    assert!(stop.load(Ordering::Relaxed));
    release.store(true, Ordering::Relaxed);
    poll_reply(&mut run, &driver);
    assert!(
        driver
            .borrow()
            .status_text()
            .contains("before completing an iteration")
    );
    assert_eq!(game(&driver.borrow()).repetition_history().len(), 2);
}

#[test]
fn a_fast_production_reply_cancels_both_timers_without_flashing_status() {
    let (mut probe, host) = probe();
    let driver = probe.driver.clone();
    let timers = probe.timers.clone();
    let mut run = host.windowless(&mut probe).unwrap();
    driver.borrow_mut().advance_computer(run.host_mut());
    assert!(!driver.borrow().status_text().contains("thinking"));
    poll_reply(&mut run, &driver);
    assert_eq!(game(&driver.borrow()).repetition_history().len(), 3);
    assert!(!driver.borrow().status_text().contains("thinking"));
    run.advance_time(SEARCH_BUDGET).unwrap();
    run.poll().unwrap();
    assert_eq!(*timers.borrow(), 0);
}

#[test]
fn undo_restart_title_and_game_end_cancel_host_timers() {
    for action in ["undo", "restart", "title", "end"] {
        let (mut probe, host) = probe();
        let driver = probe.driver.clone();
        let timers = probe.timers.clone();
        let mut run = host.windowless(&mut probe).unwrap();
        let _stop = slow_reply(&mut driver.borrow_mut(), run.host_mut());
        match action {
            "undo" => driver.borrow_mut().undo().unwrap(),
            "restart" => driver.borrow_mut().restart().unwrap(),
            "title" => driver.borrow_mut().show_title().unwrap(),
            "end" => driver
                .borrow_mut()
                .finish("White wins: Black resigned".into())
                .unwrap(),
            _ => unreachable!(),
        }
        // Synthetic input is queued through update; native input is cancelled
        // in its event callback. Either boundary must remove obsolete timers.
        run.frame().unwrap();
        run.advance_time(SEARCH_BUDGET).unwrap();
        run.poll().unwrap();
        assert_eq!(*timers.borrow(), 0, "obsolete timer after {action}");
        assert!(!driver.borrow().status_text().contains("thinking"));
        assert!(!driver.borrow().status_text().contains("error"));
    }
}

#[test]
fn load_while_thinking_cancels_timers_and_rejects_the_old_request() {
    let (mut probe, host) = probe();
    let driver = probe.driver.clone();
    let timers = probe.timers.clone();
    let root = tempfile::tempdir().unwrap();
    driver.borrow_mut().open_store(root.path().to_path_buf());
    driver.borrow_mut().save_game(1).unwrap();
    let saved = driver.borrow().saved_payload(1).unwrap();
    let mut run = host.windowless(&mut probe).unwrap();
    let stop = slow_reply(&mut driver.borrow_mut(), run.host_mut());
    let old_generation = driver.borrow().computer.as_ref().unwrap().generation;
    let old_move = game(&driver.borrow()).parse_uci("g8f6").unwrap();
    run.advance_time(Duration::from_millis(300)).unwrap();
    run.poll().unwrap();
    assert!(driver.borrow().status_text().contains("thinking"));
    driver.borrow_mut().save_game(2).unwrap();
    let thinking_save = driver.borrow().saved_payload(2).unwrap();
    assert_eq!(thinking_save, saved);
    assert!(thinking_save.result.is_none());
    assert!(
        !serde_json::to_string(&thinking_save)
            .unwrap()
            .contains("thinking")
    );
    driver.borrow_mut().load_game(1).unwrap();
    {
        let driver = driver.borrow();
        let computer = driver.computer.as_ref().unwrap();
        assert_ne!(computer.generation, old_generation);
        assert!(computer.pending.is_none());
        assert!(computer.queue.is_none());
        assert_eq!(driver.cancelled_timers.len(), 2);
        assert_eq!(game(&driver), &saved.replay().unwrap());
    }
    driver
        .borrow_mut()
        .accept_reply(old_generation, Some(old_move))
        .unwrap();
    driver.borrow_mut().drain_computer().unwrap();
    assert_eq!(game(&driver.borrow()), &saved.replay().unwrap());
    menu_key(&mut driver.borrow_mut(), "escape");
    run.frame().unwrap();
    assert!(driver.borrow().cancelled_timers.is_empty());
    run.advance_time(SEARCH_BUDGET).unwrap();
    run.poll().unwrap();
    assert_eq!(
        *timers.borrow(),
        1,
        "loaded game must retire both old timers"
    );
    assert!(!stop.load(Ordering::Relaxed));
    assert!(!driver.borrow().status_text().contains("thinking"));
    assert!(!driver.borrow().status_text().contains("error"));
}

#[test]
fn starting_lessons_and_puzzles_retires_search_timers() {
    use crate::retained::menus::MenuAction;
    for action in [MenuAction::Lesson(0), MenuAction::PuzzleBand(None)] {
        let (mut probe, host) = probe();
        let driver = probe.driver.clone();
        let timers = probe.timers.clone();
        let mut run = host.windowless(&mut probe).unwrap();
        let stop = slow_reply(&mut driver.borrow_mut(), run.host_mut());
        driver.borrow_mut().training_action(action).unwrap();
        let training_game = game(&driver.borrow()).clone();
        assert!(driver.borrow().computer.is_none());
        assert_eq!(driver.borrow().cancelled_timers.len(), 2);
        run.frame().unwrap();
        assert!(driver.borrow().cancelled_timers.is_empty());
        run.advance_time(SEARCH_BUDGET).unwrap();
        run.poll().unwrap();
        assert_eq!(*timers.borrow(), 0, "obsolete timer after {action:?}");
        assert!(!stop.load(Ordering::Relaxed));
        assert_eq!(game(&driver.borrow()), &training_game);
        assert!(!driver.borrow().status_text().contains("thinking"));
        assert!(!driver.borrow().status_text().contains("error"));
    }
}
