#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, RetainedInput};

fn fixture() -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver.show_title().unwrap();
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

#[test]
fn title_opens_lessons_with_keyboard() {
    let mut driver = fixture();
    for name in ["arrowdown", "arrowdown", "enter"] {
        key(&mut driver, name);
    }
    assert_eq!(format!("{:?}", driver.screen()), "Lessons");
    assert!(!driver.exit_requested());
}
