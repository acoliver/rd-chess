#![cfg(feature = "viewer")]
use chess3d::retained::persistence::{GamePayload, GameSettings, SavedMode};
use chess3d::{
    CameraPreset, Side,
    retained::{BoardDriver, ComputerGame, RetainedBoard, Screen, viewer_options},
    rules::ChessState,
};

fn driver(root: &std::path::Path) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.open_store(root.to_path_buf());
    driver.configure(&options).unwrap();
    driver
}

#[test]
fn replay_preserves_castling_en_passant_counters_and_repetition() {
    for moves in [
        "e2e4 e7e5 g1f3 b8c6 f1e2 g8f6 e1g1 d7d5",
        "e2e4 a7a6 e4e5 d7d5 e5d6",
        "g1f3 g8f6 f3g1 f6g8 g1f3 g8f6 f3g1 f6g8",
    ] {
        let mut game = ChessState::initial();
        for uci in moves.split_whitespace() {
            game.play(game.parse_uci(uci).unwrap()).unwrap();
        }
        let payload = GamePayload::from_game(&game, 42, SavedMode::TwoPlayer, None);
        let bytes = serde_json::to_vec(&payload).unwrap();
        let decoded: GamePayload = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(decoded.replay().unwrap(), game);
    }
}

#[test]
fn autosave_move_undo_restart_and_continue_only_for_live_games() {
    let root = tempfile::tempdir().unwrap();
    let mut first = driver(root.path());
    assert!(!first.can_continue());
    first.play_uci("e2e4").unwrap();
    let mut next = driver(root.path());
    assert!(next.can_continue());
    next.continue_game().unwrap();
    assert_eq!(next.board().unwrap().game(), first.board().unwrap().game());
    first.undo().unwrap();
    next = driver(root.path());
    next.continue_game().unwrap();
    assert_eq!(next.board().unwrap().game(), &ChessState::initial());
    first.play_uci("d2d4").unwrap();
    first.restart().unwrap();
    next = driver(root.path());
    next.continue_game().unwrap();
    assert_eq!(next.board().unwrap().game(), &ChessState::initial());
    for uci in ["f2f3", "e7e5", "g2g4", "d8h4"] {
        first.play_uci(uci).unwrap();
    }
    assert!(!driver(root.path()).can_continue());
}

#[test]
fn manual_slots_labels_and_title_preserve_current_game() {
    let root = tempfile::tempdir().unwrap();
    let mut first = driver(root.path());
    first.play_uci("e2e4").unwrap();
    first.save_game(1).unwrap();
    assert!(first.slot_labels()[0].contains("Two players, move 1"));
    first.show_title().unwrap();
    let mut next = driver(root.path());
    next.continue_game().unwrap();
    assert_eq!(
        next.board().unwrap().game().to_fen(),
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
    next.play_uci("e7e5").unwrap();
    next.load_game(1).unwrap();
    assert_eq!(
        next.board().unwrap().game().to_fen(),
        first.saved_payload(1).unwrap().replay().unwrap().to_fen()
    );
}

#[test]
fn illegal_and_truncated_files_are_visible_and_never_overwritten() {
    let root = tempfile::tempdir().unwrap();
    let mut first = driver(root.path());
    first.play_uci("e2e4").unwrap();
    first.save_game(1).unwrap();
    let path = root.path().join("slot-slot-1.rds");
    std::fs::write(&path, b"truncated").unwrap();
    assert!(first.load_game(1).is_err());
    assert!(first.status_text().contains("Storage error"));
    assert_eq!(std::fs::read(&path).unwrap(), b"truncated");
    let current = root.path().join("slot-current.rds");
    std::fs::write(&current, b"bad current").unwrap();
    let mut next = driver(root.path());
    assert!(!next.can_continue());
    assert!(next.status_text().contains("Storage error"));
    next.play_uci("d2d4").unwrap();
    assert_eq!(std::fs::read(&current).unwrap(), b"bad current");
    let mut payload =
        GamePayload::from_game(&ChessState::initial(), 42, SavedMode::TwoPlayer, None);
    payload.moves.push("e2e5".into());
    assert!(payload.replay().unwrap_err().contains("e2e5"));
}

#[test]
fn settings_persist_and_unwritable_storage_does_not_stop_play() {
    let root = tempfile::tempdir().unwrap();
    let mut first = driver(root.path());
    let settings = GameSettings {
        level: 3,
        side: 1,
        markers: false,
        overhead: true,
    };
    first.set_settings(settings).unwrap();
    assert_eq!(driver(root.path()).settings(), settings);
    let blocked = root.path().join("not-a-directory");
    std::fs::write(&blocked, b"keep").unwrap();
    let mut blocked_driver = driver(&blocked);
    blocked_driver.play_uci("e2e4").unwrap();
    assert!(blocked_driver.status_text().contains("Storage error"));
    assert_eq!(std::fs::read(blocked).unwrap(), b"keep");
}

#[test]
fn computer_mode_and_finished_result_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let mut first = driver(root.path());
    first
        .set_computer(ComputerGame::new(Side::Onyx, 3, 42).unwrap())
        .unwrap();
    first.save_game(2).unwrap();
    let mut next = driver(root.path());
    next.load_game(2).unwrap();
    let config = next.computer_game().unwrap();
    assert_eq!(
        (config.human(), config.level(), config.seed()),
        (Side::Onyx, 3, 42)
    );
    assert_eq!(next.screen(), Screen::Play);
    assert!(next.status_text().contains("Level 3"));
}
