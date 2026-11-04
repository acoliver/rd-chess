#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{
        BoardDriver, RetainedBoard,
        persistence::{GamePayload, GameSettings, SavedMode},
        viewer_options,
    },
    rules::ChessState,
};
use std::path::Path;
use worldrunner::world::worldrunner_save::{
    ApplicationStore, SettingValue, Settings, SlotId, SlotMetadata, StoreLimits,
};

fn store(root: &Path) -> ApplicationStore {
    ApplicationStore::new(
        root.to_path_buf(),
        StoreLimits {
            payload_bytes: 2_097_152,
            label_bytes: 256,
            settings_bytes: 4096,
            value_bytes: 256,
            settings_entries: 16,
        },
    )
    .unwrap()
}
fn driver(root: &Path) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.open_store(root.to_path_buf());
    driver.configure(&options).unwrap();
    driver
}
fn write(root: &Path, name: &str, bytes: &[u8]) {
    store(root)
        .write_slot(
            &SlotId::new(name).unwrap(),
            bytes,
            &SlotMetadata {
                label: "test".into(),
                payload_version: 1,
            },
        )
        .unwrap();
}

#[test]
fn invalid_json_illegal_moves_and_future_versions_preserve_files_and_live_board() {
    let root = tempfile::tempdir().unwrap();
    let mut illegal =
        GamePayload::from_game(&ChessState::initial(), 42, SavedMode::TwoPlayer, None);
    illegal.moves.push("e2e5".into());
    let mut future = illegal.clone();
    future.version = 2;
    for bytes in [
        b"{\"version\":1,".to_vec(),
        serde_json::to_vec(&illegal).unwrap(),
        serde_json::to_vec(&future).unwrap(),
    ] {
        write(root.path(), "slot-1", &bytes);
        write(root.path(), "current", &bytes);
        let path = root.path().join("slot-slot-1.rds");
        let original = std::fs::read(&path).unwrap();
        let mut driver = driver(root.path());
        assert!(!driver.can_continue());
        assert!(driver.status_text().contains("Storage error"));
        driver.play_uci("d2d4").unwrap();
        let live = driver.board().unwrap().game().clone();
        assert!(driver.load_game(1).is_err());
        assert_eq!(driver.board().unwrap().game(), &live);
        assert!(driver.save_game(1).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(
            std::fs::read(root.path().join("slot-current.rds")).unwrap(),
            original
        );
    }
}

#[test]
fn oversized_save_is_visible_without_replacement() {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "slot-1", &vec![b'x'; 1_048_577]);
    let path = root.path().join("slot-slot-1.rds");
    let bytes = std::fs::read(&path).unwrap();
    let mut driver = driver(root.path());
    assert!(driver.load_game(1).is_err());
    assert!(driver.status_text().contains("exceeds limit"));
    assert!(driver.save_game(1).is_err());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    driver.play_uci("e2e4").unwrap();
}

#[test]
fn invalid_settings_are_visible_and_retained_while_choices_apply_in_memory() {
    let root = tempfile::tempdir().unwrap();
    store(root.path())
        .write_settings(&Settings::from([(
            "version".into(),
            SettingValue::Integer(2),
        )]))
        .unwrap();
    let path = root.path().join("settings.rds");
    let bytes = std::fs::read(&path).unwrap();
    let mut driver = driver(root.path());
    assert!(driver.status_text().contains("Storage error"));
    let settings = GameSettings {
        level: 8,
        side: 2,
        markers: false,
        overhead: true,
    };
    assert!(driver.set_settings(settings).is_err());
    assert_eq!(driver.settings(), settings);
    assert_eq!(driver.camera_pose().unwrap(), CameraPreset::Overhead.pose());
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    driver.play_uci("e2e4").unwrap();
}

#[cfg(unix)]
#[test]
fn readonly_directory_preserves_existing_autosave_and_play_continues() {
    use std::os::unix::fs::PermissionsExt;
    let root = tempfile::tempdir().unwrap();
    let mut driver = driver(root.path());
    driver.play_uci("e2e4").unwrap();
    let path = root.path().join("slot-current.rds");
    let bytes = std::fs::read(&path).unwrap();
    let permissions = std::fs::metadata(root.path()).unwrap().permissions();
    std::fs::set_permissions(root.path(), std::fs::Permissions::from_mode(0o555)).unwrap();
    let played = driver.play_uci("e7e5");
    std::fs::set_permissions(root.path(), permissions).unwrap();
    played.unwrap();
    assert!(driver.status_text().contains("Storage error"));
    assert_eq!(std::fs::read(path).unwrap(), bytes);
    assert_eq!(
        driver.board().unwrap().game().side_to_move(),
        chess3d::Side::Ivory
    );
}
