use super::{assert_failure, cli};

#[test]
fn review_explain_lesson_and_puzzle_reject_every_cross_mode_pair() {
    let modes = [
        vec!["--review-moves", ""],
        vec!["--explain", "bad-fen", "e2e4"],
        vec!["--lesson", "tactics.fork"],
        vec!["--puzzle", "00008"],
        vec!["--pgn-check", "unused.pgn"],
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

#[test]
fn cpu_modes_reject_other_options_in_either_order_without_side_effects() {
    let directory = tempfile::tempdir().unwrap();
    let pgn = directory.path().join("games.pgn");
    let output = directory.path().join("unused");
    std::fs::write(&pgn, "1.e4 *").unwrap();
    for (mode, value) in [
        ("--pgn-check", pgn.to_str().unwrap()),
        ("--review-moves", ""),
    ] {
        for flags in [
            vec!["--data-dir", output.to_str().unwrap()],
            vec!["--save", output.to_str().unwrap()],
            vec!["--load", output.to_str().unwrap()],
            vec!["--capture", output.to_str().unwrap()],
            vec!["--window"],
            vec!["--view", "hero"],
            vec!["--width", "1280"],
            vec!["--height", "960"],
            vec!["--seed", "342"],
            vec!["--moves", ""],
            vec!["--play", "white", "--level", "1"],
            vec!["--level", "1"],
        ] {
            for mode_first in [true, false] {
                let mut command = cli();
                if mode_first {
                    command.args([mode, value]).args(&flags);
                } else {
                    command.args(&flags).args([mode, value]);
                }
                let result = command.output().unwrap();
                assert_failure(&result, &format!("{mode} cannot be combined"));
                assert!(result.stdout.is_empty());
                assert!(!output.exists());
                assert_eq!(std::fs::read_to_string(&pgn).unwrap(), "1.e4 *");
            }
        }
    }
}

#[test]
fn pgn_and_review_modes_are_mutually_exclusive_in_either_order() {
    for review in [vec!["--review-moves", ""], vec!["--review-fen", "bad"]] {
        for pgn_first in [true, false] {
            let mut command = cli();
            if pgn_first {
                command.args(["--pgn-check", "unused.pgn"]).args(&review);
            } else {
                command.args(&review).args(["--pgn-check", "unused.pgn"]);
            }
            let output = command.output().unwrap();
            assert_failure(&output, "--pgn-check cannot be combined");
            assert!(output.stdout.is_empty());
        }
    }
    let output = cli()
        .args([
            "--review-fen",
            chess3d::rules::INITIAL_FEN,
            "--review-moves",
            "",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(String::from_utf8_lossy(&output.stdout).contains("0/0"));
    let output = cli().arg("--help").output().unwrap();
    assert!(output.status.success());
    let help = String::from_utf8_lossy(&output.stdout);
    for flag in [
        "--pgn-check",
        "--review-moves",
        "--review-fen",
        "--data-dir",
    ] {
        assert!(help.contains(flag), "missing {flag}: {help}");
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
fn pgn_mode_excludes_every_teaching_mode_in_either_order() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("games.pgn");
    std::fs::write(&path, "1.e4 *").unwrap();
    let fen = "7k/8/5KQ1/8/8/8/8/8 w - - 0 1";
    for teaching in [
        vec!["--lesson", "rules.first-moves"],
        vec!["--puzzle", "00008"],
        vec!["--explain", fen, "g6g7"],
        vec!["--explain", fen, "g6g7", "--explain-verbose"],
    ] {
        for pgn_first in [true, false] {
            let mut command = cli();
            if pgn_first {
                command.arg("--pgn-check").arg(&path).args(&teaching);
            } else {
                command.args(&teaching).arg("--pgn-check").arg(&path);
            }
            let output = command.output().unwrap();
            assert_failure(&output, "--pgn-check cannot be combined");
            assert!(output.stdout.is_empty());
        }
    }
}

#[test]
fn lesson_and_puzzle_inspection_reject_even_default_valued_game_options() {
    let directory = tempfile::tempdir().unwrap();
    let data = directory.path().join("application-data");
    for mode in [["--lesson", "rules.first-moves"], ["--puzzle", "00008"]] {
        for extra in [
            vec!["--view", "hero"],
            vec!["--width", "1280"],
            vec!["--height", "960"],
            vec!["--data-dir", data.to_str().unwrap()],
        ] {
            for teaching_first in [true, false] {
                let mut command = cli();
                if teaching_first {
                    command.args(mode).args(&extra);
                } else {
                    command.args(&extra).args(mode);
                }
                let output = command.output().unwrap();
                assert_failure(&output, "teaching inspection cannot combine");
                assert!(output.stdout.is_empty());
                assert!(!data.exists());
            }
        }
    }
}
