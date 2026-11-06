use super::editing_tests::{click, fixture, key};
use super::{Screen, menus::MenuAction};
use worldrunner::ui::{Clipboard, ClipboardError, MemoryClipboard};

#[test]
fn computer_setup_offers_clipboard_import_as_a_local_game() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.menu_action(MenuAction::Computer).unwrap();
    let mut clipboard = MemoryClipboard::default();
    clipboard.write_text("1. d4 d5 *").unwrap();
    click(&mut driver, [400., 515.]);
    driver.flush_clipboard(&mut clipboard).unwrap();
    assert_eq!(driver.screen(), Screen::Play);
    assert!(driver.computer_game().is_none());
    assert_eq!(driver.board().unwrap().game().record().1, ["d2d4", "d7d5"]);
}

#[test]
fn pasted_game_collection_uses_selector_and_escape_returns_to_new_game_menu() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.show_title().unwrap();
    let mut clipboard = MemoryClipboard::default();
    clipboard.write_text("1. e4 *\n1. d4 *").unwrap();
    click(&mut driver, [400., 55.]);
    driver.flush_clipboard(&mut clipboard).unwrap();
    assert_eq!(driver.screen(), Screen::PgnGames);
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Title);
    assert!(driver.pgn_game_labels().is_empty());
    click(&mut driver, [400., 55.]);
    driver.flush_clipboard(&mut clipboard).unwrap();
    key(&mut driver, "arrowdown");
    key(&mut driver, "enter");
    assert_eq!(driver.board().unwrap().game().record().1, ["d2d4"]);
}

#[test]
fn game_menu_copy_writes_the_existing_export_exactly() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.play_uci("e2e4").unwrap();
    let path = driver.export_pgn().unwrap();
    let expected = std::fs::read_to_string(path).unwrap();
    key(&mut driver, "escape");
    click(&mut driver, [400., 496.]);
    let mut clipboard = MemoryClipboard::default();
    driver.flush_clipboard(&mut clipboard).unwrap();
    assert_eq!(clipboard.read_text().unwrap(), expected);
    assert!(driver.status_text().contains("PGN copied"));
}

#[test]
fn review_copy_exports_the_whole_game_not_the_displayed_position() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    for uci in ["f2f3", "e7e5", "g2g4", "d8h4"] {
        driver.play_uci(uci).unwrap();
    }
    let expected = std::fs::read_to_string(driver.export_pgn().unwrap()).unwrap();
    driver.menu_action(MenuAction::Review).unwrap();
    driver.menu_action(MenuAction::ReviewStart).unwrap();
    assert_eq!(driver.review_cursor(), Some(0));
    click(&mut driver, [560., 578.]);
    let mut clipboard = MemoryClipboard::default();
    driver.flush_clipboard(&mut clipboard).unwrap();
    assert_eq!(clipboard.read_text().unwrap(), expected);
    assert!(driver.status_text().contains("PGN copied"));
    assert_eq!(driver.review_cursor(), Some(0));
}

#[test]
fn paste_pgn_loads_a_game_and_invalid_input_never_partially_loads() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    driver.show_title().unwrap();
    let mut clipboard = MemoryClipboard::default();
    clipboard.write_text("1. e4 e5 2. Nf3 *").unwrap();
    click(&mut driver, [400., 55.]);
    driver.flush_clipboard(&mut clipboard).unwrap();
    assert_eq!(driver.screen(), Screen::Play);
    assert_eq!(
        driver.board().unwrap().game().record().1,
        ["e2e4", "e7e5", "g1f3"]
    );
    driver.show_title().unwrap();
    let before = driver.board().unwrap().game().clone();
    let payload = std::fs::read(root.path().join("slot-current.rds")).unwrap();
    for text in ["1. d4 d9 *", "1. e4 *\n1. e9 *", "", "nonsense"] {
        clipboard.write_text(text).unwrap();
        driver.menu_action(MenuAction::PastePgn).unwrap();
        driver.flush_clipboard(&mut clipboard).unwrap();
        assert_eq!(driver.screen(), Screen::Title);
        assert_eq!(driver.board().unwrap().game(), &before);
        assert_eq!(
            std::fs::read(root.path().join("slot-current.rds")).unwrap(),
            payload
        );
        assert!(driver.status_text().contains("PGN error:"));
    }
}

#[test]
fn clipboard_failures_are_visible_and_leave_the_game_unchanged() {
    struct Unavailable;
    impl Clipboard for Unavailable {
        fn read_text(&mut self) -> Result<String, ClipboardError> {
            Err(ClipboardError::Unavailable)
        }
        fn write_text(&mut self, _: &str) -> Result<(), ClipboardError> {
            Err(ClipboardError::WriteFailed)
        }
    }
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path());
    let before = driver.board().unwrap().game().clone();
    for action in [MenuAction::CopyPgn, MenuAction::PastePgn] {
        driver.menu_action(action).unwrap();
        driver.flush_clipboard(&mut Unavailable).unwrap();
        assert!(driver.status_text().contains("clipboard"));
        assert_eq!(driver.board().unwrap().game(), &before);
    }
}
