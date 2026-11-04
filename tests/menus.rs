#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Side,
    retained::{BoardDriver, RetainedBoard, Screen, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::{SceneView, Viewport},
    viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput},
};

fn fixture(fen: &str) -> BoardDriver {
    fixture_seed(fen, 42)
}
fn fixture_seed(fen: &str, seed: u64) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::from_fen(fen).unwrap(), seed).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
}
fn key(driver: &mut BoardDriver, name: &str) {
    for pressed in [true, false] {
        driver
            .handle_input(
                &RetainedInput::Key {
                    key: name.into(),
                    logical: LogicalKey::Character(name.into()),
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
fn click(driver: &mut BoardDriver, point: [f64; 2]) -> bool {
    let mut consumed = false;
    for input in [
        RetainedInput::PointerMoved(point),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ] {
        consumed = driver.handle_input(&input, None, None).unwrap();
    }
    consumed
}
fn command(driver: &mut BoardDriver, index: usize) {
    let last = match driver.screen() {
        Screen::Title | Screen::InGameMenu => 4,
        Screen::GameOver => 1,
        _ => panic!("command menu required"),
    };
    key(driver, "end");
    for _ in index..last {
        key(driver, "arrowup");
    }
    key(driver, "enter");
}

#[test]
fn title_keyboard_navigates_without_activating_and_starts_friend() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    driver.show_title().unwrap();
    key(&mut driver, "arrowdown");
    assert_eq!(driver.screen(), Screen::Title);
    key(&mut driver, "arrowup");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Play);
    assert!(driver.computer_game().is_none());
}

#[test]
fn title_pointer_and_setup_choose_black_and_scrolled_level_eight() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    driver.show_title().unwrap();
    assert!(click(&mut driver, [400., 310.]));
    assert_eq!(driver.screen(), Screen::ComputerSetup);
    key(&mut driver, "arrowdown");
    key(&mut driver, "enter");
    key(&mut driver, "tab");
    key(&mut driver, "tab");
    key(&mut driver, "end");
    assert_eq!(driver.screen(), Screen::ComputerSetup);
    key(&mut driver, "tab");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Play);
    let config = driver.computer_game().unwrap();
    assert_eq!(
        (config.human(), config.level(), config.seed()),
        (Side::Onyx, 8, 42)
    );
    assert!(driver.status_text().contains("Level 8, Soren"));
}

#[test]
fn escape_opens_menu_instead_of_exiting_and_blocks_board_input() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    key(&mut driver, "escape");
    assert!(
        !driver.exit_requested(),
        "Escape must open the in-game menu"
    );
    assert_eq!(driver.screen(), Screen::InGameMenu);
    let board = driver.board().unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let view = SceneView::new(
        options.pose,
        options.camera,
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    let point = glam::Vec2::new(40., 580.);
    for input in [
        RetainedInput::PointerMoved([f64::from(point.x), f64::from(point.y)]),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ] {
        assert!(driver.handle_input(&input, Some(&view), None).unwrap());
    }
    assert_eq!(driver.selected(), None);
    assert_eq!(driver.board().unwrap().game(), &ChessState::initial());
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Play);
}

#[test]
fn menu_undo_restart_resign_and_game_over_actions() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    driver.play_uci("e2e4").unwrap();
    key(&mut driver, "escape");
    command(&mut driver, 1);
    assert_eq!(driver.screen(), Screen::Play);
    assert_eq!(driver.board().unwrap().game(), &ChessState::initial());
    driver.play_uci("d2d4").unwrap();
    key(&mut driver, "escape");
    command(&mut driver, 2);
    assert_eq!(driver.board().unwrap().game(), &ChessState::initial());
    key(&mut driver, "escape");
    command(&mut driver, 3);
    assert_eq!(driver.screen(), Screen::GameOver);
    assert_eq!(driver.status_text(), "Black wins: White resigned");
    assert!(driver.play_uci("e2e4").is_err());
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Play);
    assert_eq!(driver.status_text(), "White to move");
    key(&mut driver, "escape");
    command(&mut driver, 3);
    command(&mut driver, 1);
    assert_eq!(driver.screen(), Screen::Title);
}

#[test]
fn claim_controls_follow_rules_and_stop_play() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    assert!(!driver.claim_available());
    assert!(!click(&mut driver, [100., 112.]));
    for _ in 0..2 {
        for uci in ["g1f3", "g8f6", "f3g1", "f6g8"] {
            driver.play_uci(uci).unwrap();
        }
    }
    assert!(driver.claim_available());
    assert!(click(&mut driver, [100., 112.]));
    assert_eq!(driver.screen(), Screen::GameOver);
    assert_eq!(driver.status_text(), "Draw: threefold repetition claimed");
    assert!(driver.play_uci("e2e4").is_err());
    let mut driver = fixture("4k3/8/8/8/8/8/8/R3K3 w - - 100 1");
    assert!(driver.claim_available());
    assert!(click(&mut driver, [100., 112.]));
    assert_eq!(driver.status_text(), "Draw: fifty-move rule claimed");
}

#[test]
fn automatic_game_over_new_game_and_title() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    for uci in ["f2f3", "e7e5", "g2g4", "d8h4"] {
        driver.play_uci(uci).unwrap();
    }
    assert_eq!(driver.screen(), Screen::GameOver);
    assert_eq!(driver.status_text(), "Checkmate: Black wins");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Play);
    assert_eq!(driver.board().unwrap().game(), &ChessState::initial());
    key(&mut driver, "escape");
    command(&mut driver, 4);
    assert_eq!(driver.screen(), Screen::Title);
    assert_eq!(driver.board().unwrap().game(), &ChessState::initial());
}

#[test]
fn setup_back_escape_quit_and_seeded_random_side() {
    for back_button in [false, true] {
        let mut driver = fixture(chess3d::rules::INITIAL_FEN);
        driver.show_title().unwrap();
        command(&mut driver, 1);
        if back_button {
            assert!(click(&mut driver, [550., 515.]));
        } else {
            key(&mut driver, "escape");
        }
        assert_eq!(driver.screen(), Screen::Title);
    }
    for (seed, expected) in [(42, Side::Ivory), (44, Side::Onyx), (42, Side::Ivory)] {
        let mut driver = fixture_seed(chess3d::rules::INITIAL_FEN, seed);
        driver.show_title().unwrap();
        command(&mut driver, 1);
        key(&mut driver, "end");
        for _ in 0..9 {
            key(&mut driver, "tab");
        }
        key(&mut driver, "enter");
        let config = driver.computer_game().unwrap();
        assert_eq!((config.human(), config.seed()), (expected, seed));
    }
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    driver.show_title().unwrap();
    command(&mut driver, 4);
    assert!(driver.exit_requested());
}

#[test]
fn a_black_human_can_resign_while_the_computer_has_the_turn() {
    let mut driver = fixture(chess3d::rules::INITIAL_FEN);
    driver
        .set_computer(chess3d::retained::ComputerGame::new(Side::Onyx, 1, 42).unwrap())
        .unwrap();
    key(&mut driver, "escape");
    command(&mut driver, 3);
    assert_eq!(driver.screen(), Screen::GameOver);
    assert_eq!(driver.status_text(), "White wins: Black resigned");
    assert!(driver.play_uci("e2e4").is_err());
}
