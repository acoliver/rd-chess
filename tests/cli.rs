#[test]
fn saved_game_directory_flag_is_preserved_and_excludes_pgn_mode() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("player-data");
    let pgn = directory.path().join("game.pgn");
    std::fs::write(&pgn, "1.e4 *").unwrap();
    let output = cli().arg("--data-dir").arg(&data).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(
        !data.exists(),
        "CPU inspection must not open the player store"
    );
    assert_failure(
        &cli().arg("--data-dir").output().unwrap(),
        "--data-dir requires a value",
    );
    for pgn_first in [true, false] {
        let mut command = cli();
        if pgn_first {
            command
                .arg("--pgn-check")
                .arg(&pgn)
                .arg("--data-dir")
                .arg(&data);
        } else {
            command
                .arg("--data-dir")
                .arg(&data)
                .arg("--pgn-check")
                .arg(&pgn);
        }
        let output = command.output().unwrap();
        assert_failure(&output, "--pgn-check cannot be combined");
        assert!(output.stdout.is_empty());
        assert!(!data.exists());
    }
    let output = cli().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--data-dir PATH"));
    assert!(help.contains("--pgn-check"));
}

#[test]
fn pgn_check_bounds_files_and_never_prints_partial_success() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("input.pgn");
    std::fs::write(&path, "1.e4 * 1.e4 e4 *").unwrap();
    let output = cli().arg("--pgn-check").arg(&path).output().unwrap();
    assert_failure(&output, "game 2, ply 2");
    assert!(output.stdout.is_empty());
    let file = std::fs::File::create(&path).unwrap();
    file.set_len(u64::try_from(chess3d::rules::pgn::MAX_INPUT_BYTES).unwrap() + 1)
        .unwrap();
    let output = cli().arg("--pgn-check").arg(&path).output().unwrap();
    assert_failure(&output, "PGN byte limit");
    std::fs::write(&path, [0xff]).unwrap();
    assert!(
        !cli()
            .arg("--pgn-check")
            .arg(&path)
            .output()
            .unwrap()
            .status
            .success()
    );
}

#[test]
fn pgn_check_reports_each_final_position_without_a_gpu() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("games.pgn");
    std::fs::write(&path, "1.e4 e5 *\n1.f3 e5 2.g4 Qh4# 0-1").unwrap();
    let output = cli().arg("--pgn-check").arg(&path).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut state = chess3d::rules::ChessState::initial();
    state.play(state.parse_uci("e2e4").unwrap()).unwrap();
    state.play(state.parse_uci("e7e5").unwrap()).unwrap();
    assert!(stdout.contains(&format!("Game 1: {} *", state.to_fen())));
    assert!(stdout.lines().nth(1).unwrap().ends_with(" 0-1"));
    assert_eq!(stdout.lines().count(), 2);
    assert!(
        String::from_utf8_lossy(&cli().arg("--help").output().unwrap().stdout)
            .contains("--pgn-check")
    );
}

#[test]
fn pgn_check_rejects_bad_input_and_other_modes_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("bad.pgn");
    let save = directory.path().join("never.json");
    std::fs::write(&input, "1.e4 e4 *").unwrap();
    let output = cli().arg("--pgn-check").arg(&input).output().unwrap();
    assert_failure(&output, "game 1, ply 2");
    assert!(output.stdout.is_empty());
    let output = cli()
        .arg("--pgn-check")
        .arg(&input)
        .arg("--save")
        .arg(&save)
        .output()
        .unwrap();
    assert_failure(&output, "--pgn-check cannot be combined");
    assert!(!save.exists());
    let missing = cli()
        .args(["--pgn-check", "no-such-pgn-file"])
        .output()
        .unwrap();
    assert!(!missing.status.success());
}

#[test]
fn computer_arguments_are_validated_without_launching_a_window() {
    for (args, message) in [
        (vec!["--play", "green"], "--play must be white or black"),
        (vec!["--level", "0"], "--level must be 1..=8"),
        (vec!["--level", "9"], "--level must be 1..=8"),
        (vec!["--level", "1"], "--level requires --play"),
        (vec!["--play", "white"], "--play requires --level"),
        (
            vec!["--play", "black", "--level", "1"],
            "--play requires --window",
        ),
    ] {
        assert_failure(&cli().args(args).output().unwrap(), message);
    }
    let output = cli().arg("--help").output().unwrap();
    let help = String::from_utf8_lossy(&output.stdout);
    assert!(help.contains("--play white|black"));
    assert!(help.contains("--level 1..8"));
}

#[test]
fn uci_sequence_saves_the_rules_result_and_illegal_input_writes_nothing() {
    let directory = tempfile::tempdir().unwrap();
    let save = directory.path().join("moves.json");
    let sequence = "e2e4 d7d5 e4d5 g8f6";
    let result = cli()
        .args(["--moves", sequence, "--save"])
        .arg(&save)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let mut game = chess3d::rules::ChessState::initial();
    for uci in sequence.split_whitespace() {
        game.play(game.parse_uci(uci).unwrap()).unwrap();
    }
    assert_eq!(Position::load(&save).unwrap(), game.to_position(342));
    let original = std::fs::read(&save).unwrap();
    let result = cli()
        .args(["--moves", "e2e4 e7e4", "--save"])
        .arg(&save)
        .output()
        .unwrap();
    assert_failure(&result, "invalid UCI move 'e7e4'");
    assert_eq!(std::fs::read(&save).unwrap(), original);
    assert_failure(
        &cli()
            .args(["--moves", "e2e4", "--load", "unused.json"])
            .output()
            .unwrap(),
        "cannot use --load or lineup",
    );
    assert_failure(
        &cli()
            .args(["--moves", "e2e4", "--view", "lineup"])
            .output()
            .unwrap(),
        "cannot use --load or lineup",
    );
}

use chess3d::{Position, Square};
use std::process::{Command, Output};

fn cli() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_chess3d"));
    command
        .env_remove("DISPLAY")
        .env_remove("WAYLAND_DISPLAY")
        .env_remove("WAYLAND_SOCKET")
        .env("VK_DRIVER_FILES", "/nonexistent/chess3d-no-gpu.json");
    command
}
fn assert_failure(output: &Output, message: &str) {
    assert!(!output.status.success(), "unexpected success: {output:?}");
    assert!(
        String::from_utf8_lossy(&output.stderr).contains(message),
        "{output:?}"
    );
}

#[cfg(not(feature = "viewer"))]
#[test]
fn window_requires_explicit_build_feature_before_saving() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("position.json");
    let output = cli()
        .arg("--window")
        .arg("--save")
        .arg(&path)
        .output()
        .unwrap();
    assert_failure(&output, "--features viewer -- --window");
    assert!(!path.exists());
}

#[cfg(all(feature = "viewer", target_os = "linux"))]
#[test]
fn window_without_a_display_reports_launch_failure_without_panicking() {
    for view in ["hero", "overhead", "lineup"] {
        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("input.json");
        let output = directory.path().join("output.json");
        let mut position = Position::initial(77);
        position.pieces.pop();
        position.pieces[0].square = Square::new(0, 3).unwrap();
        position.save_atomic(&input).unwrap();
        let result = cli()
            .arg("--window")
            .args(["--view", view])
            .arg("--load")
            .arg(&input)
            .arg("--save")
            .arg(&output)
            .output()
            .unwrap();
        assert_failure(&result, "EventLoop");
        let stderr = String::from_utf8_lossy(&result.stderr);
        assert!(
            stderr.contains("DISPLAY") || stderr.contains("Wayland"),
            "{result:?}"
        );
        assert!(!stderr.contains("panicked"), "{result:?}");
        assert_eq!(Position::load(&output).unwrap(), position);
    }
}

#[test]
fn capture_and_window_are_rejected_before_writing() {
    let directory = tempfile::tempdir().unwrap();
    let save = directory.path().join("position.json");
    let capture = directory.path().join("capture.png");
    let result = cli()
        .arg("--window")
        .arg("--capture")
        .arg(&capture)
        .arg("--save")
        .arg(&save)
        .output()
        .unwrap();
    assert_failure(&result, "--capture and --window cannot be combined");
    assert!(!save.exists());
    assert!(!capture.exists());
}

#[test]
fn help_and_position_inspection_need_no_gpu() {
    let help = cli().arg("--help").output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    for documented in ["--capture", "--window", "--features viewer", "R/Home"] {
        assert!(help.contains(documented), "missing {documented}: {help}");
    }
    let ready = cli().output().unwrap();
    assert!(ready.status.success());
    assert!(String::from_utf8_lossy(&ready.stdout).contains("32 pieces, seed 342"));
    let directory = tempfile::tempdir().unwrap();
    let save = directory.path().join("initial.json");
    let result = cli().arg("--save").arg(&save).output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let saved = Position::load(&save).unwrap();
    assert_eq!(saved.pieces.len(), 32);
    assert_eq!(saved, Position::initial(342));
    assert_eq!(
        saved,
        Position::load(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/positions/initial.json")
        )
        .unwrap()
    );
}

#[test]
fn saves_and_loads_an_edited_captured_position_without_a_gpu() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("nested/position.json");
    let result = cli()
        .args(["--seed", "18446744073709551615", "--save"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let mut position = Position::load(&path).unwrap();
    assert_eq!(position.seed, u64::MAX);
    position.pieces[0].square = Square::new(0, 3).unwrap();
    position.pieces.pop();
    position.save_atomic(&path).unwrap();
    let result = cli()
        .arg("--load")
        .arg(&path)
        .arg("--save")
        .arg(&path)
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    assert_eq!(Position::load(&path).unwrap(), position);
    assert!(String::from_utf8_lossy(&result.stdout).contains("31 pieces"));
}

#[test]
fn invalid_arguments_are_rejected_before_side_effects() {
    for (args, message) in [
        (vec!["--view", "sideways"], "--view must be"),
        (vec!["--width", "63"], "dimensions must be"),
        (vec!["--height", "4097"], "dimensions must be"),
        (vec!["--load"], "requires a value"),
        (vec!["--seed"], "requires a value"),
        (vec!["--nonsense"], "unknown argument"),
    ] {
        assert_failure(&cli().args(args).output().unwrap(), message);
    }
}

#[test]
fn seed_with_load_is_not_silently_ignored() {
    assert_failure(
        &cli()
            .args(["--load", "unused.json", "--seed", "2"])
            .output()
            .unwrap(),
        "--seed cannot be combined with --load",
    );
}

#[test]
fn capture_cannot_overwrite_a_position_document() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("position.json");
    let original = Position::initial(2).to_json().unwrap();
    std::fs::write(&path, &original).unwrap();
    let result = cli()
        .arg("--load")
        .arg(&path)
        .arg("--capture")
        .arg(&path)
        .output()
        .unwrap();
    assert_failure(&result, "capture path must differ");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), original);
}

#[test]
fn invalid_load_cannot_replace_an_existing_save() {
    let directory = tempfile::tempdir().unwrap();
    let input = directory.path().join("invalid.json");
    let output = directory.path().join("position.json");
    std::fs::write(&input, "{}").unwrap();
    let original = Position::initial(91).to_json().unwrap();
    std::fs::write(&output, &original).unwrap();
    let result = cli()
        .arg("--load")
        .arg(&input)
        .arg("--save")
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert_eq!(std::fs::read_to_string(&output).unwrap(), original);
}

#[cfg(unix)]
#[test]
fn capture_and_save_cannot_alias_through_a_symlinked_parent() {
    let directory = tempfile::tempdir().unwrap();
    let real = directory.path().join("real");
    let alias = directory.path().join("alias");
    std::fs::create_dir(&real).unwrap();
    std::os::unix::fs::symlink(&real, &alias).unwrap();
    let result = cli()
        .arg("--save")
        .arg(real.join("new.json"))
        .arg("--capture")
        .arg(alias.join("new.json"))
        .output()
        .unwrap();
    assert_failure(&result, "capture path must differ");
    assert!(!real.join("new.json").exists());
}
