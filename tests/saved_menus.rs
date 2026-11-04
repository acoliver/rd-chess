#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Side,
    retained::{BoardDriver, RetainedBoard, Screen, persistence::GameSettings, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput};

fn fixture(root: &std::path::Path) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.open_store(root.to_path_buf());
    driver.configure(&options).unwrap();
    driver
}
fn click(driver: &mut BoardDriver, point: [f64; 2]) {
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
        assert!(driver.handle_input(&input, None, None).unwrap());
    }
}
fn key(driver: &mut BoardDriver, key: &str) {
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
fn title_continue_is_absent_until_a_resumable_save_exists() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.show_title().unwrap();
    assert!(!driver.can_continue());
    click(&mut driver, [204., 510.]);
    assert_eq!(
        driver.screen(),
        Screen::Slots,
        "without Continue, Saved games is the first secondary row"
    );
    key(&mut driver, "escape");
    click(&mut driver, [400., 258.]);
    driver.play_uci("e2e4").unwrap();
    let game = driver.board().unwrap().game().clone();
    driver.show_title().unwrap();
    click(&mut driver, [204., 510.]);
    assert_eq!(driver.screen(), Screen::Play);
    assert_eq!(driver.board().unwrap().game(), &game);
}

#[test]
fn native_settings_choices_persist_and_set_new_computer_defaults() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.show_title().unwrap();
    click(&mut driver, [400., 510.]);
    assert_eq!(driver.screen(), Screen::Settings);
    click(&mut driver, [290., 212.]);
    click(&mut driver, [400., 292.]);
    click(&mut driver, [547., 372.]);
    click(&mut driver, [547., 452.]);
    let settings = GameSettings {
        level: 3,
        side: 1,
        markers: false,
        overhead: true,
    };
    assert_eq!(driver.settings(), settings);
    assert_eq!(driver.camera_pose().unwrap(), CameraPreset::Overhead.pose());
    let mut restarted = fixture(root.path());
    assert_eq!(restarted.settings(), settings);
    restarted.show_title().unwrap();
    click(&mut restarted, [400., 310.]);
    click(&mut restarted, [250., 515.]);
    assert_eq!(
        (
            restarted.computer_game().unwrap().human(),
            restarted.computer_game().unwrap().level()
        ),
        (Side::Onyx, 3)
    );
}

#[test]
fn title_and_pause_offer_manual_save_load_with_generated_labels() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.play_uci("e2e4").unwrap();
    key(&mut driver, "escape");
    click(&mut driver, [400., 455.]);
    assert_eq!(driver.screen(), Screen::Slots);
    for y in [160., 272., 384.] {
        click(&mut driver, [250., y]);
    }
    assert!(
        driver
            .slot_labels()
            .iter()
            .all(|label| label == "Two players, move 1")
    );
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::InGameMenu);
    driver.show_title().unwrap();
    click(&mut driver, [400., 510.]);
    click(&mut driver, [250., 272.]);
    click(&mut driver, [550., 272.]);
    assert_eq!(driver.screen(), Screen::Play);
    assert_eq!(driver.board().unwrap().game().side_to_move(), Side::Onyx);
}

#[test]
fn resignation_result_persists_and_finished_current_has_no_continue() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.play_uci("e2e4").unwrap();
    key(&mut driver, "escape");
    key(&mut driver, "end");
    key(&mut driver, "arrowup");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::GameOver);
    assert_eq!(driver.status_text(), "White wins: Black resigned");
    driver.save_game(3).unwrap();
    assert_eq!(
        driver.saved_payload(3).unwrap().result.as_deref(),
        Some("White wins: Black resigned")
    );
    let mut next = fixture(root.path());
    assert!(!next.can_continue());
    next.load_game(3).unwrap();
    assert_eq!(next.screen(), Screen::GameOver);
    assert_eq!(next.status_text(), "White wins: Black resigned");
    assert!(next.play_uci("e7e5").is_err());
}
