#[test]
fn computer_reply_announces_the_committed_move_after_the_human_move() {
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
    driver.semantic_update().unwrap();
    driver.play_uci("e2e4").unwrap();
    let human = driver.semantic_update().unwrap().unwrap();
    assert_eq!(human.announcements[0].text, "White pawn from e2 to e4");
    let reply = game(&driver).parse_uci("g8f6").unwrap();
    inject(&mut driver, &mut host, Some(reply));
    wait(&mut driver);
    let computer = driver.semantic_update().unwrap().unwrap();
    assert_eq!(computer.announcements[0].text, "Black knight from g8 to f6");
    assert!(
        driver
            .semantic_update()
            .unwrap()
            .unwrap()
            .announcements
            .is_empty()
    );
}

#[test]
fn why_control_shows_verified_sentences_after_a_computer_reply() {
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
    driver.play_uci("e2e4").unwrap();
    driver.advance_computer(&mut host);
    wait(&mut driver);
    let mut before = game(&driver).clone();
    before.undo().unwrap();
    let mv = before.parse_uci("b8c6").unwrap();
    let expected =
        crate::teaching::explain_reply(&before, mv, crate::teaching::Budget::default()).unwrap();
    menu_key(&mut driver, "escape");
    for input in [
        RetainedInput::PointerMoved([400., 508.]),
        button(true),
        button(false),
    ] {
        driver.handle_input(&input, None, None).unwrap();
    }
    for sentence in expected.sentences {
        assert!(
            driver.panel_text().contains(&sentence),
            "{}",
            driver.panel_text()
        );
    }
    driver.undo().unwrap();
    assert!(driver.panel_text().is_empty());
}

use super::*;
use crate::{CameraPreset, PieceKind, Square, retained::RetainedBoard, rules::INITIAL_FEN};
use std::time::{Duration, Instant};
use worldrunner::scene::{
    gpu::{SceneView, Viewport},
    viewer::{MouseButton, RetainedInput},
};

mod timers;

#[test]
fn timer_admission_failure_does_not_submit_or_leak_a_partial_pair() {
    use worldrunner::scene::viewer::MAX_HOST_TIMERS;
    for available in [0, 1] {
        let (mut driver, mut host) = setup(INITIAL_FEN, Side::Onyx);
        for _ in 0..MAX_HOST_TIMERS - available {
            host.timer_at(host.timer_now() + SEARCH_BUDGET).unwrap();
        }
        driver.advance_computer(&mut host);
        assert!(driver.computer.as_ref().unwrap().pending.is_none());
        assert!(driver.status_text().contains("host timer limit exceeded"));
        if available == 1 {
            host.timer_at(host.timer_now()).unwrap();
        }
        assert!(host.timer_at(host.timer_now()).is_err());
    }
}

#[test]
fn a_computer_mate_delivered_with_the_menu_open_shows_game_over() {
    let (mut driver, mut host) = setup("7k/5Q2/5K2/8/8/8/8/8 w - - 0 1", Side::Onyx);
    menu_key(&mut driver, "escape");
    let chess_move = game(&driver).parse_uci("f7g7").unwrap();
    inject(&mut driver, &mut host, Some(chess_move));
    wait(&mut driver);
    assert_eq!(driver.screen(), crate::retained::Screen::GameOver);
    assert_eq!(driver.status_text(), "Checkmate: White wins");
    for input in [
        RetainedInput::PointerMoved([400., 508.]),
        button(true),
        button(false),
    ] {
        driver.handle_input(&input, None, None).unwrap();
    }
    assert!(
        !driver.panel_text().is_empty(),
        "the mating reply can also be explained"
    );
}

fn menu_key(driver: &mut BoardDriver, key: &str) {
    use worldrunner::scene::viewer::{KeyCode, LogicalKey};
    for pressed in [true, false] {
        driver
            .handle_input(
                &RetainedInput::Key {
                    key: key.into(),
                    logical: LogicalKey::Character(key.into()),
                    physical: KeyCode::Enter.into(),
                    pressed,
                    repeat: false,
                },
                None,
                None,
            )
            .unwrap();
    }
}

#[test]
fn setup_black_human_schedules_the_opening_at_the_selected_level() {
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
    driver.show_title().unwrap();
    for key in ["arrowdown", "enter", "arrowdown", "tab", "tab", "enter"] {
        menu_key(&mut driver, key);
    }
    // Black selected; level one selected. Tab through the remaining rows to Start.
    for _ in 0..8 {
        menu_key(&mut driver, "tab");
    }
    menu_key(&mut driver, "enter");
    let config = driver.computer_game().unwrap();
    assert_eq!((config.human(), config.level()), (Side::Onyx, 1));
    driver.advance_computer(&mut host);
    assert!(driver.computer.as_ref().unwrap().pending.is_some());
    wait(&mut driver);
    assert_eq!(game(&driver).side_to_move(), Side::Onyx);
    assert_eq!(game(&driver).repetition_history().len(), 2);
}

#[test]
fn title_discards_an_outstanding_result_before_starting_a_new_game() {
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Onyx);
    let reply = game(&driver).parse_uci("e2e4").unwrap();
    inject(&mut driver, &mut host, Some(reply));
    // A work notification can arrive after the menu transition.
    for key in ["escape", "end", "enter"] {
        menu_key(&mut driver, key);
    }
    assert!(driver.computer.is_none());
    driver.drain_computer().unwrap();
    assert_eq!(game(&driver), &ChessState::initial());
    menu_key(&mut driver, "home");
    menu_key(&mut driver, "enter");
    assert_eq!(driver.screen(), crate::retained::Screen::Play);
    assert!(driver.computer.is_none());
    assert_eq!(game(&driver), &ChessState::initial());
}

fn setup(fen: &str, human: Side) -> (BoardDriver, RetainedHost) {
    let board = RetainedBoard::new(ChessState::from_fen(fen).unwrap(), 342).unwrap();
    let options = super::super::viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let host = RetainedHost::new(board.scene().clone(), options.clone()).unwrap();
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
        .set_computer(ComputerGame::new(human, 1, 42).unwrap())
        .unwrap();
    (driver, host)
}
fn game(driver: &BoardDriver) -> &ChessState {
    driver.board().unwrap().game()
}
fn wait(driver: &mut BoardDriver) {
    let clock = Instant::now();
    let mut watchdog = crate::retained::watchdog::ProgressWatchdog::new(
        Duration::ZERO,
        true,
        "computer pending result",
    );
    while driver.computer.as_ref().unwrap().pending.is_some() {
        driver.drain_computer().unwrap();
        watchdog
            .observe(
                clock.elapsed(),
                driver.computer.as_ref().unwrap().pending.is_some(),
            )
            .unwrap();
        std::thread::yield_now();
    }
}
fn inject(driver: &mut BoardDriver, host: &mut RetainedHost, result: Option<LegalMove>) {
    let computer = driver.computer.as_mut().unwrap();
    computer.queue = Some(host.work_queue(1, 1).unwrap());
    let generation = computer.generation;
    let ticket = computer
        .queue
        .as_ref()
        .unwrap()
        .submit(move |_| Reply {
            generation,
            chess_move: result,
        })
        .unwrap();
    computer.pending = Some(Request::new(ticket, generation, host));
}

#[test]
fn replies_once_and_is_deterministic_for_seed_and_level() {
    let mut replies = Vec::new();
    for _ in 0..2 {
        let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
        driver.play_uci("e2e4").unwrap();
        let expected = crate::search::choose_move(game(&driver), &LADDER[0], move_seed(42, 1));
        assert_eq!(expected.to_uci(), "b8c6", "Linux window smoke reference");
        driver.advance_computer(&mut host);
        assert!(!driver.status_text().contains("thinking"));
        wait(&mut driver);
        assert_eq!(game(&driver).repetition_history().len(), 3);
        replies.push(game(&driver).clone());
        let mut expected_state = ChessState::initial();
        expected_state
            .play(expected_state.parse_uci("e2e4").unwrap())
            .unwrap();
        expected_state.play(expected).unwrap();
        assert_eq!(game(&driver), &expected_state);
        for _ in 0..3 {
            driver.advance_computer(&mut host);
            driver.drain_computer().unwrap();
        }
        assert_eq!(game(&driver).repetition_history().len(), 3);
    }
    assert_eq!(replies[0], replies[1]);
}

#[test]
fn undo_and_restart_invalidate_stale_results_and_cancel_tickets() {
    for restart in [false, true] {
        let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
        driver.play_uci("e2e4").unwrap();
        let old = driver.computer.as_ref().unwrap().generation;
        let reply = game(&driver).parse_uci("e7e5").unwrap();
        driver.advance_computer(&mut host);
        if restart {
            driver.restart().unwrap();
        } else {
            driver.undo().unwrap();
        }
        assert!(driver.computer.as_ref().unwrap().pending.is_none());
        driver.accept_reply(old, Some(reply)).unwrap();
        driver.drain_computer().unwrap();
        assert_eq!(game(&driver), &ChessState::initial());
        assert!(!driver.status_text().contains("error"));
    }
}

#[test]
fn illegal_and_missing_results_and_panics_surface_without_mutating() {
    for missing in [false, true] {
        let (mut driver, mut host) = setup(INITIAL_FEN, Side::Onyx);
        let mut other = ChessState::initial();
        other.play(other.parse_uci("e2e4").unwrap()).unwrap();
        let illegal = other.parse_uci("e7e5").unwrap();
        inject(&mut driver, &mut host, (!missing).then_some(illegal));
        wait(&mut driver);
        assert_eq!(game(&driver), &ChessState::initial());
        assert!(driver.status_text().contains("Computer error"));
        driver.advance_computer(&mut host);
        assert!(driver.computer.as_ref().unwrap().pending.is_none());
    }
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Onyx);
    let computer = driver.computer.as_mut().unwrap();
    computer.queue = Some(host.work_queue(1, 1).unwrap());
    let ticket = computer
        .queue
        .as_ref()
        .unwrap()
        .submit(|_| panic!("test job"))
        .unwrap();
    computer.pending = Some(Request::new(ticket, computer.generation, &mut host));
    wait(&mut driver);
    assert!(driver.status_text().contains("panicked"));
}

#[test]
fn computer_promotion_is_one_batch_without_dialog() {
    let (mut driver, mut host) = setup("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", Side::Onyx);
    let promotion = game(&driver).parse_uci("a7a8n").unwrap();
    inject(&mut driver, &mut host, Some(promotion));
    wait(&mut driver);
    assert!(!driver.promotion_pending());
    assert_eq!(
        game(&driver).piece_at(Square::new(0, 7).unwrap()),
        Some((Side::Ivory, PieceKind::Knight))
    );
    assert_eq!(driver.queue().borrow().len(), 1);
}

#[test]
fn undo_returns_to_last_human_decision_before_or_after_reply() {
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
    driver.play_uci("e2e4").unwrap();
    driver.undo().unwrap();
    assert_eq!(game(&driver), &ChessState::initial());
    driver.play_uci("e2e4").unwrap();
    driver.advance_computer(&mut host);
    wait(&mut driver);
    driver.undo().unwrap();
    assert_eq!(game(&driver), &ChessState::initial());
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Onyx);
    driver.advance_computer(&mut host);
    wait(&mut driver);
    let opening = game(&driver).clone();
    assert!(
        driver.undo().is_err(),
        "no human decision before computer opening"
    );
    assert_eq!(game(&driver), &opening);
    let chess_move = game(&driver).legal_moves()[0];
    driver.commit_move(chess_move).unwrap();
    driver.advance_computer(&mut host);
    wait(&mut driver);
    driver.undo().unwrap();
    assert_eq!(game(&driver), &opening);
}

#[test]
fn board_clicks_are_blocked_during_computer_turn_but_orbit_works() {
    let (mut driver, _) = setup(INITIAL_FEN, Side::Onyx);
    let view = SceneView::new(
        CameraPreset::Overhead.pose(),
        driver
            .board()
            .unwrap()
            .scene()
            .effects()
            .unwrap()
            .camera()
            .to_owned(),
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    let screen = view
        .resolved_view()
        .world_to_screen(glam::Vec3::new(0.5, -2.5, 0.05))
        .unwrap();
    let start = [f64::from(screen.x), f64::from(screen.y)];
    for input in [
        RetainedInput::PointerMoved(start),
        button(true),
        button(false),
    ] {
        driver.handle_input(&input, Some(&view), None).unwrap();
    }
    assert_eq!(driver.selected(), None);
    for input in [
        button(true),
        RetainedInput::PointerMoved([start[0] + 30., start[1] + 20.]),
        button(false),
    ] {
        driver.handle_input(&input, Some(&view), None).unwrap();
    }
    assert_ne!(driver.camera_pose().unwrap(), CameraPreset::Overhead.pose());
    assert_eq!(game(&driver), &ChessState::initial());
}
fn button(pressed: bool) -> RetainedInput {
    RetainedInput::Button {
        button: MouseButton::Left,
        pressed,
    }
}

#[test]
fn terminal_states_do_not_submit_and_levels_are_bounded() {
    assert!(ComputerGame::new(Side::Ivory, 0, 1).is_err());
    assert!(ComputerGame::new(Side::Ivory, 9, 1).is_err());
    let (mut driver, mut host) = setup("7k/6Q1/5K2/8/8/8/8/8 b - - 0 1", Side::Ivory);
    driver.advance_computer(&mut host);
    assert!(driver.computer.as_ref().unwrap().pending.is_none());
    assert!(driver.status_text().contains("Checkmate"));
}

#[test]
fn unrelated_host_timer_does_not_stop_a_pending_search() {
    let (mut driver, mut host) = setup(INITIAL_FEN, Side::Onyx);
    let chess_move = game(&driver).parse_uci("e2e4").unwrap();
    let computer = driver.computer.as_mut().unwrap();
    computer.queue = Some(host.work_queue(1, 1).unwrap());
    let stop = Arc::new(AtomicBool::new(false));
    let worker_stop = stop.clone();
    let generation = computer.generation;
    let ticket = computer
        .queue
        .as_ref()
        .unwrap()
        .submit(move |cancel| {
            while !worker_stop.load(Ordering::Relaxed) && !cancel.is_cancelled() {
                std::thread::yield_now();
            }
            Reply {
                generation,
                chess_move: Some(chess_move),
            }
        })
        .unwrap();
    computer.pending = Some(Request {
        ticket,
        generation,
        timers: SearchTimers::new(&mut host).unwrap(),
        stop: stop.clone(),
    });
    let unrelated = host.timer_at(host.timer_now()).unwrap();
    driver.computer_timer(&unrelated);
    driver.advance_computer(&mut host);
    assert!(!stop.load(Ordering::Relaxed));
    stop.store(true, Ordering::Relaxed);
    wait(&mut driver);
    assert_eq!(game(&driver).repetition_history().len(), 2);
    assert!(!driver.status_text().contains("error"));
}

#[test]
fn closed_queue_is_visible_and_draw_claim_does_not_schedule() {
    let (mut driver, host) = setup(INITIAL_FEN, Side::Onyx);
    let mut host = host;
    driver.computer.as_mut().unwrap().queue = Some(host.work_queue(1, 1).unwrap());
    drop(host);
    let board = driver.board().unwrap();
    let mut other = RetainedHost::new(
        board.scene().clone(),
        super::super::viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]),
    )
    .unwrap();
    driver.advance_computer(&mut other);
    assert!(driver.status_text().contains("closed"));
    let (mut driver, mut host) = setup("4k3/8/8/8/8/8/8/R3K3 w - - 100 1", Side::Onyx);
    driver.advance_computer(&mut host);
    assert!(driver.status_text().starts_with("Draw:"));
    assert!(driver.computer.as_ref().unwrap().pending.is_none());
}

#[test]
fn undo_restart_and_host_shutdown_cancel_running_jobs() {
    use std::sync::mpsc;
    for action in ["undo", "restart", "title", "close"] {
        let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
        driver.play_uci("e2e4").unwrap();
        let (started, running) = mpsc::channel();
        let (stopped, cancelled) = mpsc::channel();
        let computer = driver.computer.as_mut().unwrap();
        let generation = computer.generation;
        computer.queue = Some(host.work_queue(1, 1).unwrap());
        let ticket = computer
            .queue
            .as_ref()
            .unwrap()
            .submit(move |cancel| {
                started.send(()).unwrap();
                while !cancel.is_cancelled() {
                    std::thread::yield_now();
                }
                stopped.send(()).unwrap();
                Reply {
                    generation,
                    chess_move: None,
                }
            })
            .unwrap();
        computer.pending = Some(Request::new(ticket, generation, &mut host));
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        match action {
            "undo" => driver.undo().unwrap(),
            "restart" => driver.restart().unwrap(),
            "title" => {
                menu_key(&mut driver, "escape");
                menu_key(&mut driver, "end");
                menu_key(&mut driver, "enter");
                assert_eq!(driver.screen(), crate::retained::Screen::Title);
                assert!(driver.computer.is_none());
                assert_eq!(game(&driver), &ChessState::initial());
            }
            "close" => drop(host),
            _ => unreachable!(),
        }
        cancelled.recv_timeout(Duration::from_secs(5)).unwrap();
        driver.drain_computer().unwrap();
        assert!(!driver.status_text().contains("error"));
    }
}
