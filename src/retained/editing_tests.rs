use super::{BoardDriver, RetainedBoard, Screen, menus::MenuAction};
use crate::{CameraPreset, rules::ChessState};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput};
use worldrunner::simulation::TextInput;

mod gpu;

#[test]
fn pointer_drag_selects_the_native_field_text_for_replacement() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.save_game(1).unwrap();
    slots(&mut driver);
    click(&mut driver, [400., 204.]);
    input(&mut driver, &RetainedInput::PointerMoved([122., 290.]));
    input(
        &mut driver,
        &RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
    );
    input(&mut driver, &RetainedInput::PointerMoved([660., 290.]));
    input(
        &mut driver,
        &RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    );
    input(
        &mut driver,
        &RetainedInput::Text(TextInput::Commit("Dragged opening".into())),
    );
    key(&mut driver, "enter");
    assert_eq!(driver.slot_labels()[0], "Dragged opening");
}

pub(super) fn fixture(root: &std::path::Path) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = super::viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.open_store(root.to_path_buf());
    driver.configure(&options).unwrap();
    driver
}
fn input(driver: &mut BoardDriver, input: &RetainedInput) {
    driver.handle_input(input, None, None).unwrap();
}
fn held(driver: &mut BoardDriver, key: &str, pressed: bool) {
    input(
        driver,
        &RetainedInput::Key {
            key: key.into(),
            logical: LogicalKey::Character(key.into()),
            physical: KeyCode::Enter.into(),
            pressed,
            repeat: false,
        },
    );
}
pub(super) fn key(driver: &mut BoardDriver, key: &str) {
    held(driver, key, true);
    held(driver, key, false);
}
pub(super) fn click(driver: &mut BoardDriver, point: [f64; 2]) {
    input(driver, &RetainedInput::PointerMoved(point));
    for pressed in [true, false] {
        input(
            driver,
            &RetainedInput::Button {
                button: MouseButton::Left,
                pressed,
            },
        );
    }
}
fn replace(driver: &mut BoardDriver, text: &str) {
    key(driver, "home");
    held(driver, "shift", true);
    key(driver, "end");
    held(driver, "shift", false);
    input(driver, &RetainedInput::Text(TextInput::Commit(text.into())));
}
fn slots(driver: &mut BoardDriver) {
    driver.menu_action(MenuAction::Slots).unwrap();
}

#[test]
fn rename_commit_trims_and_survives_slot_load_and_store_reopen() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.play_uci("e2e4").unwrap();
    driver.save_game(1).unwrap();
    slots(&mut driver);
    click(&mut driver, [400., 204.]);
    replace(&mut driver, "  Opening practice  ");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Slots);
    assert_eq!(driver.slot_labels()[0], "Opening practice");
    let mut reopened = fixture(root.path());
    assert_eq!(reopened.slot_labels()[0], "Opening practice");
    reopened.load_game(1).unwrap();
    assert_eq!(reopened.saved_payload(1).unwrap().moves, ["e2e4"]);
    assert_eq!(reopened.slot_labels()[0], "Opening practice");
}

#[test]
fn rename_escape_preserves_label_and_record_bytes() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.save_game(1).unwrap();
    let path = root.path().join("slot-slot-1.rds");
    let before = std::fs::read(&path).unwrap();
    slots(&mut driver);
    click(&mut driver, [400., 204.]);
    replace(&mut driver, "Cancelled");
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Slots);
    assert_eq!(driver.slot_labels()[0], "Two players, move 0");
    assert_eq!(std::fs::read(path).unwrap(), before);
}

#[test]
fn rename_refuses_empty_and_overlong_labels_without_writing() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.save_game(1).unwrap();
    let path = root.path().join("slot-slot-1.rds");
    let before = std::fs::read(&path).unwrap();
    slots(&mut driver);
    click(&mut driver, [400., 204.]);
    for (text, message) in [("   ".to_owned(), "empty"), ("x".repeat(25), "24")] {
        replace(&mut driver, &text);
        key(&mut driver, "enter");
        assert!(
            driver.status_text().contains(message),
            "{}",
            driver.status_text()
        );
        assert_eq!(std::fs::read(&path).unwrap(), before);
    }
    replace(&mut driver, &"x".repeat(24));
    key(&mut driver, "enter");
    assert_eq!(driver.slot_labels()[0], "x".repeat(24));
}

#[test]
fn new_save_edits_generated_label_before_writing_and_can_cancel() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.play_uci("e2e4").unwrap();
    slots(&mut driver);
    click(&mut driver, [250., 160.]);
    assert!(driver.saved_payload(1).is_err(), "save must wait for Enter");
    key(&mut driver, "end");
    input(
        &mut driver,
        &RetainedInput::Text(TextInput::Commit("!".into())),
    );
    key(&mut driver, "enter");
    assert_eq!(driver.slot_labels()[0], "Two players, move 1!");
    click(&mut driver, [250., 272.]);
    key(&mut driver, "escape");
    assert!(driver.saved_payload(2).is_err());
    assert_eq!(driver.screen(), Screen::Slots);
}

#[test]
fn old_payload_without_label_uses_its_generated_label() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    let payload = super::persistence::GamePayload::from_game(
        &ChessState::initial(),
        42,
        super::persistence::SavedMode::TwoPlayer,
        None,
    );
    let mut old = serde_json::to_value(payload).unwrap();
    old.as_object_mut().unwrap().remove("label");
    let store = worldrunner::world::worldrunner_save::ApplicationStore::new(
        root.path().to_path_buf(),
        worldrunner::world::worldrunner_save::StoreLimits {
            payload_bytes: 1_048_576,
            label_bytes: 256,
            settings_bytes: 4096,
            value_bytes: 256,
            settings_entries: 16,
        },
    )
    .unwrap();
    store
        .write_slot(
            &worldrunner::world::worldrunner_save::SlotId::new("slot-1").unwrap(),
            &serde_json::to_vec(&old).unwrap(),
            &worldrunner::world::worldrunner_save::SlotMetadata {
                label: "Two players, move 0".into(),
                payload_version: 1,
            },
        )
        .unwrap();
    driver.open_store(root.path().to_path_buf());
    driver.load_game(1).unwrap();
    assert_eq!(driver.slot_labels()[0], "Two players, move 0");
    assert_eq!(
        driver.saved_payload(1).unwrap().label(),
        "Two players, move 0"
    );
}

#[test]
fn keyboard_only_save_and_rename_path_matches_window_smoke() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.play_uci("e2e4").unwrap();
    key(&mut driver, "escape");
    key(&mut driver, "home");
    for _ in 0..5 {
        key(&mut driver, "tab");
    }
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Slots);
    key(&mut driver, "enter");
    key(&mut driver, "enter");
    assert_eq!(driver.slot_labels()[0], "Two players, move 1");
    key(&mut driver, "tab");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::SaveLabel);
    replace(&mut driver, "Smoke opening");
    assert_eq!(driver.ui.as_ref().unwrap().label_value(), "Smoke opening");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Slots, "{}", driver.status_text());
    assert_eq!(driver.slot_labels()[0], "Smoke opening");
    key(&mut driver, "escape");
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Play);
}

#[test]
fn ascii_preedit_waits_for_commit_and_unsupported_text_is_reported() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.save_game(1).unwrap();
    slots(&mut driver);
    click(&mut driver, [400., 204.]);
    replace(&mut driver, "Opening");
    input(
        &mut driver,
        &RetainedInput::Text(TextInput::Preedit {
            text: "!".into(),
            cursor: Some((1, 1)),
        }),
    );
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::SaveLabel);
    assert_eq!(driver.slot_labels()[0], "Two players, move 0");
    input(
        &mut driver,
        &RetainedInput::Text(TextInput::Commit("!".into())),
    );
    input(
        &mut driver,
        &RetainedInput::Text(TextInput::Commit("名".into())),
    );
    assert!(driver.status_text().contains("ASCII"));
    key(&mut driver, "enter");
    assert_eq!(driver.slot_labels()[0], "Opening!");
}
