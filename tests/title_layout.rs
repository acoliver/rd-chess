use std::process::Command;

fn cli() -> Command {
    Command::new(env!("CARGO_BIN_EXE_chess3d"))
}

#[cfg(feature = "viewer")]
#[test]
fn title_inspection_is_windowless_and_uses_requested_dimensions() {
    for [width, height] in [[960, 720], [1200, 900]] {
        let output = cli()
            .args([
                "--inspect-title",
                "--width",
                &width.to_string(),
                "--height",
                &height.to_string(),
            ])
            .env_remove("DISPLAY")
            .env_remove("WAYLAND_DISPLAY")
            .env("WGPU_BACKEND", "invalid-no-gpu-for-inspection")
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
        let actual: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(
            actual,
            chess3d::retained::title_layout([width, height]).unwrap()
        );
        for label in ["Continue", "Play a friend", "Lessons", "Puzzles", "Quit"] {
            assert!(actual["entries"][label].is_array());
        }
    }
}

#[cfg(feature = "viewer")]
#[test]
fn title_inspection_rejects_other_modes_before_side_effects() {
    let root = tempfile::tempdir().unwrap();
    let path = root.path().join("must-not-exist");
    for flags in [
        vec!["--save", path.to_str().unwrap()],
        vec!["--capture", path.to_str().unwrap()],
        vec!["--data-dir", path.to_str().unwrap()],
        vec!["--window"],
        vec!["--puzzle", "00008"],
        vec!["--explain", "bad-fen", "e2e4"],
    ] {
        for inspection_first in [false, true] {
            let mut command = cli();
            if inspection_first {
                command.arg("--inspect-title").args(&flags);
            } else {
                command.args(&flags).arg("--inspect-title");
            }
            let output = command.output().unwrap();
            assert!(!output.status.success());
            assert!(String::from_utf8_lossy(&output.stderr).contains("cannot be combined"));
            assert!(output.stdout.is_empty());
            assert!(!path.exists());
        }
    }
}

#[cfg(not(feature = "viewer"))]
#[test]
fn title_inspection_requires_viewer() {
    let output = cli().arg("--inspect-title").output().unwrap();
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("requires the optional viewer feature")
    );
}
