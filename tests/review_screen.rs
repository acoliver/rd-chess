#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput};

fn fixture() -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
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
        driver.handle_input(&input, None, None).unwrap();
    }
}
fn finished(driver: &mut BoardDriver) {
    for uci in ["f2f3", "e7e5", "g2g4", "d8h4"] {
        driver.play_uci(uci).unwrap();
    }
}
#[test]
fn result_panel_opens_review_and_escape_restores_the_finished_record() {
    let mut driver = fixture();
    finished(&mut driver);
    let original = driver.board().unwrap().game().clone();
    let result = driver.status_text();
    click(&mut driver, [250., 455.]);
    assert_eq!(format!("{:?}", driver.screen()), "Review");
    assert!(driver.status_text().contains("Analysing move 1 of 4"));
    key(&mut driver, "escape");
    assert_eq!(driver.board().unwrap().game(), &original);
    assert_eq!(driver.status_text(), result);
}

#[test]
fn saved_review_browser_preserves_every_store_byte_and_result() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture();
    driver.open_store(root.path().to_path_buf());
    finished(&mut driver);
    driver.save_game(1).unwrap();
    let payload = serde_json::to_vec(&driver.saved_payload(1).unwrap()).unwrap();
    let before = store_bytes(root.path());
    driver.show_title().unwrap();
    click(&mut driver, [400., 378.]);
    assert_eq!(driver.screen(), chess3d::retained::Screen::ReviewGames);
    click(&mut driver, [400., 240.]);
    assert_eq!(driver.screen(), chess3d::retained::Screen::Review);
    click(&mut driver, [526., 140.]);
    click(&mut driver, [674., 140.]);
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), chess3d::retained::Screen::ReviewGames);
    assert_eq!(
        serde_json::to_vec(&driver.saved_payload(1).unwrap()).unwrap(),
        payload
    );
    assert_eq!(store_bytes(root.path()), before);
}
fn store_bytes(root: &std::path::Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect()
}

#[test]
fn resignation_and_terminal_draws_offer_review() {
    for fen in [
        "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
    ] {
        let board = RetainedBoard::new(ChessState::from_fen(fen).unwrap(), 42).unwrap();
        let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
        let mut driver = BoardDriver::with_board(board);
        driver.configure(&options).unwrap();
        let result = driver.status_text();
        click(&mut driver, [250., 455.]);
        assert_eq!(driver.screen(), chess3d::retained::Screen::Review);
        key(&mut driver, "escape");
        assert_eq!(driver.status_text(), result);
    }
    let mut driver = fixture();
    key(&mut driver, "escape");
    for name in ["end", "arrowup", "enter"] {
        key(&mut driver, name);
    }
    let result = driver.status_text();
    assert!(result.contains("resigned"));
    click(&mut driver, [250., 455.]);
    assert_eq!(driver.screen(), chess3d::retained::Screen::Review);
    key(&mut driver, "escape");
    assert_eq!(driver.status_text(), result);
}
