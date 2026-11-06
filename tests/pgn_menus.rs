#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Side,
    retained::{BoardDriver, ComputerGame, RetainedBoard, Screen, viewer_options},
    rules::{
        ChessState,
        pgn::{PgnResult, parse},
    },
};
use std::{fs, path::Path};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput};

fn fixture(root: &Path, fen: &str) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::from_fen(fen).unwrap(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.open_store(root.to_path_buf());
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
        assert!(driver.handle_input(&input, None, None).unwrap());
    }
}
fn export(driver: &mut BoardDriver) {
    if driver.screen() == Screen::Play {
        key(driver, "escape");
    }
    click(driver, [550., 455.]);
}
fn open_import(driver: &mut BoardDriver) {
    driver.show_title().unwrap();
    click(
        driver,
        [400., if driver.can_continue() { 378. } else { 332. }],
    );
    assert_eq!(driver.screen(), Screen::Slots);
    click(driver, [400., 466.]);
}
fn files(root: &Path) -> Vec<std::path::PathBuf> {
    let mut paths: Vec<_> = fs::read_dir(root.join("exports"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .collect();
    paths.sort();
    paths
}

#[test]
fn export_menu_round_trips_roster_moves_and_never_overwrites() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    driver
        .set_computer(ComputerGame::new(Side::Ivory, 2, 42).unwrap())
        .unwrap();
    driver.play_uci("e2e4").unwrap();
    driver.play_uci("e7e5").unwrap();
    let expected = driver.board().unwrap().game().clone();
    export(&mut driver);
    let first = files(root.path()).pop().expect("export created a PGN");
    let original = fs::read_to_string(&first).unwrap();
    let games = parse(&original).unwrap();
    assert_eq!(games[0].state(), &expected);
    assert_eq!(games[0].result, PgnResult::Unfinished);
    for (tag, value) in [
        ("Event", "Casual game"),
        ("Site", "rd-chess"),
        ("Round", "-"),
        ("White", "Player"),
        ("Black", "Rowan"),
    ] {
        assert_eq!(games[0].tags[tag], value);
    }
    let date = &games[0].tags["Date"];
    assert_eq!(date.len(), 10);
    assert!(!date.contains('?'));
    assert!(driver.status_text().contains(first.to_str().unwrap()));
    export(&mut driver);
    assert_eq!(files(root.path()).len(), 2);
    assert_eq!(fs::read_to_string(first).unwrap(), original);
}

#[test]
fn finished_export_retains_declared_result_and_nonstandard_initial_fen() {
    let root = tempfile::tempdir().unwrap();
    let fen = "4k3/8/8/8/8/8/8/R3K3 w - - 0 1";
    let mut driver = fixture(root.path(), fen);
    driver.play_uci("a1a2").unwrap();
    key(&mut driver, "escape");
    key(&mut driver, "end");
    key(&mut driver, "arrowup");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::GameOver);
    click(&mut driver, [550., 455.]);
    let path = files(root.path()).pop().unwrap();
    let games = parse(&fs::read_to_string(path).unwrap()).unwrap();
    assert_eq!(games[0].initial_fen(), fen);
    assert_eq!(games[0].result, PgnResult::WhiteWin);
    assert_eq!(games[0].state(), driver.board().unwrap().game());
}

#[test]
fn import_lists_files_shows_parser_errors_and_preserves_the_live_game() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("exports")).unwrap();
    let broken = "1. e9 *";
    fs::write(root.path().join("exports/broken.pgn"), broken).unwrap();
    fs::write(root.path().join("exports/ignore.txt"), "ignored").unwrap();
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    open_import(&mut driver);
    assert_eq!(driver.pgn_files(), &["broken.pgn"]);
    assert!(
        driver
            .status_text()
            .contains(root.path().join("exports").to_str().unwrap())
    );
    let before = driver.board().unwrap().game().clone();
    key(&mut driver, "enter");
    assert!(
        driver
            .status_text()
            .contains(&parse(broken).unwrap_err().to_string())
    );
    assert_eq!(driver.board().unwrap().game(), &before);
}

#[test]
fn import_selects_second_game_with_result_save_and_complete_undo_history() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("exports")).unwrap();
    let text = "1. e4 *\n\n[White \"Second\"]\n1. d4 {center} (1. e4) d5 2. c4 1/2-1/2\n";
    fs::write(root.path().join("exports/two.pgn"), text).unwrap();
    let expected = parse(text).unwrap().remove(1);
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    open_import(&mut driver);
    key(&mut driver, "enter");
    assert_eq!(driver.pgn_game_labels().len(), 2);
    key(&mut driver, "arrowdown");
    key(&mut driver, "enter");
    assert_eq!(driver.board().unwrap().game(), expected.state());
    assert_eq!(driver.screen(), Screen::GameOver);
    assert!(driver.status_text().contains("ignored"));
    driver.save_game(1).unwrap();
    assert_eq!(
        driver.saved_payload(1).unwrap().moves,
        ["d2d4", "d7d5", "c2c4"]
    );
    let mut next = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    next.load_game(1).unwrap();
    assert_eq!(next.board().unwrap().game(), expected.state());
    for name in ["u", "backspace", "u"] {
        key(&mut next, name);
    }
    assert_eq!(next.board().unwrap().game(), &ChessState::initial());
}

#[test]
fn import_refuses_parent_paths_and_symlinks_outside_data_root() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    let before = driver.board().unwrap().game().clone();
    for name in [
        "../outside.pgn",
        "/outside.pgn",
        "sub/game.pgn",
        "..\\outside.pgn",
    ] {
        assert!(driver.import_pgn_file(name).is_err(), "accepted {name}");
        assert!(driver.status_text().contains("file name"));
    }
    assert_eq!(driver.board().unwrap().game(), &before);
}

#[cfg(unix)]
#[test]
fn import_and_export_refuse_symlink_escapes() {
    use std::os::unix::fs::symlink;
    let root = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    fs::write(outside.path().join("game.pgn"), "1. e4 *").unwrap();
    symlink(outside.path(), root.path().join("exports")).unwrap();
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    export(&mut driver);
    assert!(driver.status_text().contains("directory"));
    assert_eq!(fs::read_dir(outside.path()).unwrap().count(), 1);
    assert!(driver.import_pgn_file("game.pgn").is_err());
    fs::remove_file(root.path().join("exports")).unwrap();
    fs::create_dir(root.path().join("exports")).unwrap();
    symlink(
        outside.path().join("game.pgn"),
        root.path().join("exports/link.pgn"),
    )
    .unwrap();
    assert!(driver.import_pgn_file("link.pgn").is_err());
}

#[test]
fn single_game_import_is_playable_saveable_and_undoable_from_its_fen() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("exports")).unwrap();
    let fen = "4k3/8/8/8/8/8/8/R3K3 w - - 0 1";
    let text = format!("[SetUp \"1\"]\n[FEN \"{fen}\"]\n1. Ra2 *");
    fs::write(root.path().join("exports/study.PGN"), text).unwrap();
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    open_import(&mut driver);
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Play);
    assert!(driver.computer_game().is_none());
    driver.save_game(2).unwrap();
    assert_eq!(driver.saved_payload(2).unwrap().initial_fen, fen);
    assert_eq!(driver.saved_payload(2).unwrap().moves, ["a1a2"]);
    driver.play_uci("e8e7").unwrap();
    driver.undo().unwrap();
    driver.undo().unwrap();
    assert_eq!(driver.board().unwrap().game().to_fen(), fen);
    let mut next = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    next.load_game(2).unwrap();
    next.undo().unwrap();
    assert_eq!(next.board().unwrap().game().to_fen(), fen);
}

#[test]
fn import_back_refreshes_files_and_scrolled_keyboard_selection_reaches_last_file() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    open_import(&mut driver);
    assert!(driver.pgn_files().is_empty());
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::Slots);
    for index in 0..12 {
        fs::write(
            root.path().join(format!("exports/{index:02}.pgn")),
            "1. e4 *\n1. d4 *",
        )
        .unwrap();
    }
    click(&mut driver, [400., 466.]);
    assert_eq!(driver.pgn_files().len(), 12);
    key(&mut driver, "end");
    key(&mut driver, "arrowup");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), Screen::PgnGames);
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::PgnImport);
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Slots);
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Title);
}

#[test]
fn import_rejects_invalid_later_games_and_oversized_empty_or_non_utf8_files() {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("exports")).unwrap();
    let text = "1. e4 *\n1. e9 *";
    let cases = [
        ("late.pgn", text.as_bytes().to_vec()),
        ("empty.pgn", Vec::new()),
        ("utf8.pgn", vec![255]),
        (
            "big.pgn",
            vec![b' '; chess3d::rules::pgn::MAX_INPUT_BYTES + 1],
        ),
    ];
    let mut driver = fixture(root.path(), chess3d::rules::INITIAL_FEN);
    driver.play_uci("d2d4").unwrap();
    let before = driver.board().unwrap().game().clone();
    for (name, bytes) in cases {
        fs::write(root.path().join("exports").join(name), bytes).unwrap();
        assert!(driver.import_pgn_file(name).is_err());
        assert!(driver.status_text().contains("PGN error:"));
        assert_eq!(driver.board().unwrap().game(), &before);
    }
    driver.import_pgn_file("late.pgn").unwrap_err();
    assert!(
        driver
            .status_text()
            .contains(&parse(text).unwrap_err().to_string())
    );
}

#[test]
fn export_automatic_mate_and_draw_report_the_rules_result() {
    for (fen, moves, result) in [
        (
            chess3d::rules::INITIAL_FEN,
            "f2f3 e7e5 g2g4 d8h4",
            PgnResult::BlackWin,
        ),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 1", "", PgnResult::Draw),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut driver = fixture(root.path(), fen);
        for uci in moves.split_whitespace() {
            driver.play_uci(uci).unwrap();
        }
        click(&mut driver, [550., 455.]);
        let path = files(root.path()).pop().unwrap();
        let games = parse(&fs::read_to_string(path).unwrap()).unwrap();
        assert_eq!(games[0].result, result);
    }
}
