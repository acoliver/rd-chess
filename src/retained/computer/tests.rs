use super::*;
use crate::{CameraPreset, PieceKind, Square, retained::RetainedBoard, rules::INITIAL_FEN};
use std::time::Duration;
use worldrunner::scene::{
    gpu::{SceneView, Viewport},
    viewer::{MouseButton, RetainedInput},
};

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
    let deadline = Instant::now() + Duration::from_secs(30);
    while driver.computer.as_ref().unwrap().pending.is_some() {
        assert!(Instant::now() < deadline, "computer result did not arrive");
        driver.drain_computer().unwrap();
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
    computer.pending = Some(Request::new(ticket, generation));
}

#[test]
fn replies_once_and_is_deterministic_for_seed_and_level() {
    let mut replies = Vec::new();
    for _ in 0..2 {
        let (mut driver, mut host) = setup(INITIAL_FEN, Side::Ivory);
        driver.play_uci("e2e4").unwrap();
        let expected = crate::search::choose_move(game(&driver), &LADDER[0], move_seed(42, 1));
        assert_eq!(expected.to_uci(), "g8f6", "Linux window smoke reference");
        driver.advance_computer(&mut host);
        assert_eq!(driver.status_text(), "Computer (Level 1, Pip) is thinking");
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
    computer.pending = Some(Request::new(ticket, computer.generation));
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
fn elapsed_budget_signals_stop_and_accepts_completed_result() {
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
        deadline: Instant::now(),
        stop: stop.clone(),
    });
    driver.advance_computer(&mut host);
    assert!(stop.load(Ordering::Relaxed));
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
    for action in ["undo", "restart", "close"] {
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
        computer.pending = Some(Request::new(ticket, generation));
        running.recv_timeout(Duration::from_secs(5)).unwrap();
        match action {
            "undo" => driver.undo().unwrap(),
            "restart" => driver.restart().unwrap(),
            "close" => drop(host),
            _ => unreachable!(),
        }
        cancelled.recv_timeout(Duration::from_secs(5)).unwrap();
        driver.drain_computer().unwrap();
        assert!(!driver.status_text().contains("error"));
    }
}
