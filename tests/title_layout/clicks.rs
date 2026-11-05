use chess3d::{
    CameraPreset,
    retained::{BoardDriver, RetainedBoard, Screen, title_layout, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::viewer::{MouseButton, RetainedInput};

fn click_entry(driver: &mut BoardDriver, layout: &serde_json::Value, label: &str) {
    let [left, top, right, bottom]: [f64; 4] =
        serde_json::from_value(layout["entries"][label].clone()).unwrap();
    let point = [left.midpoint(right).round(), top.midpoint(bottom).round()];
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

#[test]
fn smoke_clicks_use_resolved_nine_entry_title_rows() {
    for viewport in [[960, 720], [1200, 900]] {
        let root = tempfile::tempdir().unwrap();
        let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
        let options = viewer_options(board.scene(), CameraPreset::Overhead, viewport);
        let mut driver = BoardDriver::with_board(board);
        driver.open_store(root.path().to_path_buf());
        driver.configure(&options).unwrap();
        driver.play_uci("d2d4").unwrap();
        driver.show_title().unwrap();
        assert!(driver.can_continue());
        let layout = title_layout(viewport).unwrap();
        click_entry(&mut driver, &layout, "Play a friend");
        assert_eq!(driver.screen(), Screen::Play);
        driver.play_uci("e2e4").unwrap();
        let moved = driver.board().unwrap().game().clone();
        driver.show_title().unwrap();
        assert!(driver.can_continue());
        click_entry(&mut driver, &layout, "Continue");
        assert_eq!(driver.screen(), Screen::Play);
        assert_eq!(driver.board().unwrap().game(), &moved);
        driver.show_title().unwrap();
        click_entry(&mut driver, &layout, "Puzzles");
        assert_eq!(driver.screen(), Screen::Puzzles);
    }
}
