#[test]
fn review_explain_lesson_and_puzzle_reject_every_cross_mode_pair() {
    let modes = [
        vec!["--review-moves", ""],
        vec!["--explain", "bad-fen", "e2e4"],
        vec!["--lesson", "tactics.fork"],
        vec!["--puzzle", "00008"],
    ];
    for (i, first) in modes.iter().enumerate() {
        for (j, second) in modes.iter().enumerate() {
            if i != j {
                let output = cli().args(first).args(second).output().unwrap();
                assert_failure(&output, "cannot be combined");
            }
        }
    }
    assert_failure(
        &cli().args(&modes[1]).args(&modes[1]).output().unwrap(),
        "--explain cannot be combined",
    );
    for mode in [&modes[0], &modes[2], &modes[3]] {
        for option in [
            vec!["--width", "1280"],
            vec!["--height", "960"],
            vec!["--view", "hero"],
        ] {
            let output = cli().args(mode).args(option).output().unwrap();
            assert!(!output.status.success(), "{output:?}");
        }
    }
    for mode in [&modes[1], &modes[2], &modes[3]] {
        let output = cli()
            .args(mode)
            .args(["--review-fen", "bad"])
            .output()
            .unwrap();
        assert!(!output.status.success(), "{output:?}");
    }
}

#[cfg(not(debug_assertions))]
#[test]
#[ignore = "production budget: run cargo test --release --test cli review_moves_is_cpu_only -- --ignored"]
fn review_moves_is_cpu_only_and_reports_the_refutation() {
    let output = cli()
        .args([
            "--review-moves",
            "d1d4",
            "--review-fen",
            "4k3/8/8/8/r7/8/8/3QK3 w - - 0 1",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8(output.stdout).unwrap();
    for expected in [
        "240000",
        "Qd4",
        "blunder",
        "Rxd4",
        "Teachable moments",
        "ply 1",
    ] {
        assert!(text.contains(expected), "missing {expected}: {text}");
    }
}

#[test]
fn review_arguments_and_records_fail_before_writing() {
    for (args, message) in [
        (
            vec!["--review-fen", "bad"],
            "--review-fen requires --review-moves",
        ),
        (
            vec!["--review-moves", "e2e5"],
            "invalid review UCI move 'e2e5'",
        ),
        (vec!["--review-moves", "", "--review-fen", "bad"], "FEN"),
        (
            vec!["--review-moves", "", "--window"],
            "--review-moves cannot be combined",
        ),
        (
            vec!["--review-moves", "", "--moves", ""],
            "--review-moves cannot be combined",
        ),
        (vec!["--review", "unused.pgn"], "unknown argument --review"),
    ] {
        assert_failure(&cli().args(args).output().unwrap(), message);
    }
    let directory = tempfile::tempdir().unwrap();
    let save = directory.path().join("review.json");
    let output = cli()
        .args(["--review-moves", "e2e4", "--save"])
        .arg(&save)
        .output()
        .unwrap();
    assert_failure(&output, "--review-moves cannot be combined");
    assert!(!save.exists());
    let output = cli().args(["--review-moves", ""]).output().unwrap();
    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("0/0"));
}

#[test]
fn every_value_flag_is_recognized_before_requiring_its_value() {
    for flag in [
        "--load",
        "--data-dir",
        "--save",
        "--capture",
        "--view",
        "--width",
        "--height",
        "--seed",
        "--moves",
        "--review-moves",
        "--review-fen",
        "--play",
        "--level",
        "--lesson",
        "--puzzle",
    ] {
        assert_failure(&cli().arg(flag).output().unwrap(), "requires a value");
    }
}

#[test]
fn data_directory_and_position_flags_parse_together_without_gpu() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("application-data");
    let save = directory.path().join("position.json");
    let result = cli()
        .arg("--data-dir")
        .arg(&data)
        .arg("--save")
        .arg(&save)
        .args([
            "--moves", "e2e4", "--view", "overhead", "--width", "800", "--height", "600", "--seed",
            "42",
        ])
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let mut game = chess3d::rules::ChessState::initial();
    game.play(game.parse_uci("e2e4").unwrap()).unwrap();
    assert_eq!(Position::load(&save).unwrap(), game.to_position(42));
    assert!(
        !data.exists(),
        "CPU inspection must not open the game store"
    );
}

#[test]
fn teaching_modes_are_mutually_exclusive_and_all_appear_in_help() {
    let fen = "r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1";
    for mode in ["--lesson", "--puzzle"] {
        let id = if mode == "--lesson" {
            "tactics.fork"
        } else {
            "00008"
        };
        for args in [
            vec![mode, id, "--explain", fen, "b5c7"],
            vec!["--explain", fen, "b5c7", mode, id],
        ] {
            assert_failure(
                &cli().args(args).output().unwrap(),
                "--explain cannot be combined",
            );
        }
        assert_failure(
            &cli()
                .args([mode, id, "--explain-verbose"])
                .output()
                .unwrap(),
            "--explain-verbose requires --explain",
        );
        assert_failure(&cli().arg(mode).output().unwrap(), "requires a value");
    }
    let help = cli().arg("--help").output().unwrap();
    assert!(help.status.success());
    let help = String::from_utf8_lossy(&help.stdout);
    for flag in ["--lesson", "--puzzle", "--explain", "--explain-verbose"] {
        assert!(help.contains(flag), "{help}");
    }
}

#[test]
fn explanation_diagnostics_require_the_verbose_flag() {
    let fen = "r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1";
    let plain = cli().args(["--explain", fen, "b5c7"]).output().unwrap();
    assert!(plain.status.success(), "{plain:?}");
    let plain = String::from_utf8(plain.stdout).unwrap();
    assert!(plain.contains("wins a rook in this line."), "{plain}");
    assert!(plain.contains("Evidence: Nc7+ Kf8 Nxa8"), "{plain}");
    assert!(!plain.contains("centipawn"), "{plain}");
    for args in [
        vec!["--explain", fen, "b5c7", "--explain-verbose"],
        vec!["--explain-verbose", "--explain", fen, "b5c7"],
    ] {
        let verbose = cli().args(args).output().unwrap();
        assert!(verbose.status.success(), "{verbose:?}");
        let verbose = String::from_utf8(verbose.stdout).unwrap();
        assert!(verbose.starts_with(&plain), "{verbose}");
        assert!(verbose.contains("Analysis limits:"), "{verbose}");
        assert!(
            verbose.contains("957/100000 nodes; verified depth 3"),
            "{verbose}"
        );
        assert!(
            verbose.contains("Exchange facts cover legal captures"),
            "{verbose}"
        );
    }
    for (args, message) in [
        (
            vec!["--explain-verbose"],
            "--explain-verbose requires --explain",
        ),
        (
            vec![
                "--explain",
                fen,
                "b5c7",
                "--explain-verbose",
                "--explain-verbose",
            ],
            "may only be specified once",
        ),
        (
            vec!["--explain", fen, "b5c7", "--explain-verbose", "--window"],
            "--explain cannot be combined",
        ),
    ] {
        assert_failure(&cli().args(args).output().unwrap(), message);
    }
    let help = cli().arg("--help").output().unwrap();
    assert!(String::from_utf8_lossy(&help.stdout).contains("--explain-verbose"));
}

#[test]
fn quiet_move_comparisons_name_alternatives_and_keep_both_lines() {
    let opening = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    let output = cli().args(["--explain", opening, "g1f3"]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("No tactical issue found within the analysis limits."),
        "{text}"
    );
    assert!(text.contains("Line after Nf3: Nf3"), "{text}");
    assert!(
        !text.contains("search prefers") && !text.contains("scores better"),
        "{text}"
    );
    let fen = "3r3k/8/8/8/8/8/8/3Q2NK w - - 0 1";
    let output = cli().args(["--explain", fen, "g1f3"]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(
        text.contains("The search prefers Qxd8+: this line scores better in the current search."),
        "{text}"
    );
    assert!(text.contains("Line after Qxd8+: Qxd8+"), "{text}");
    assert!(text.contains("Line after Nf3: Nf3"), "{text}");
    assert!(!text.contains("\nLine:"), "{text}");
}

#[test]
fn explanations_are_cpu_only_and_validate_both_arguments() {
    let fen = "7k/8/5KQ1/8/8/8/8/8 w - - 0 1";
    let output = cli().args(["--explain", fen, "g6g7"]).output().unwrap();
    assert!(output.status.success(), "{output:?}");
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Qg7# is checkmate."), "{text}");
    assert!(text.contains("Line after Qg7#: Qg7#"), "{text}");
    assert!(!text.contains("nodes") && !text.contains("depth"), "{text}");
    assert!(!text.contains("Exchange facts cover"), "{text}");
    for (args, message) in [
        (vec!["--explain", fen], "--explain requires FEN and UCI"),
        (vec!["--explain", "invalid", "g6g7"], "FEN"),
        (vec!["--explain", fen, "g6g9"], "move notation"),
        (
            vec!["--explain", fen, "g6g7", "--window"],
            "--explain cannot be combined",
        ),
    ] {
        assert_failure(&cli().args(args).output().unwrap(), message);
    }
}

#[test]
fn teaching_cli_inspects_lessons_and_setup_positions_without_gpu() {
    let result = cli()
        .args(["--lesson", "rules.first-moves"])
        .output()
        .unwrap();
    assert!(result.status.success(), "{result:?}");
    let text = String::from_utf8_lossy(&result.stdout);
    assert!(text.contains("Pawns, knights and kings"));
    assert!(text.contains("e2e4"));
    let pack = chess3d::teaching::PuzzlePack::bundled().unwrap();
    let puzzle = &pack.puzzles()[0];
    let result = cli().args(["--puzzle", &puzzle.id]).output().unwrap();
    assert!(result.status.success(), "{result:?}");
    let text = String::from_utf8_lossy(&result.stdout);
    assert!(text.contains(&puzzle.after_setup().unwrap().to_fen()));
    assert!(text.contains("Solution:"));
    for args in [
        vec!["--lesson", "unknown"],
        vec!["--puzzle", "unknown"],
        vec!["--lesson", "rules.first-moves", "--puzzle", &puzzle.id],
    ] {
        assert!(!cli().args(args).output().unwrap().status.success());
    }
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("untouched.json");
    let result = cli()
        .args(["--lesson", "rules.first-moves", "--save"])
        .arg(&path)
        .output()
        .unwrap();
    assert_failure(&result, "teaching inspection");
    assert!(!path.exists());
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
