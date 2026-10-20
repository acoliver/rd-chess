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

#[cfg(feature = "viewer")]
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
