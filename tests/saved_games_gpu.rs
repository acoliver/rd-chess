//! Required same-device relaunch and resumed computer delivery without windows.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Side,
    retained::{BoardDriver, ComputerGame, RetainedBoard, capture_frame, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::viewer::{MouseButton, RetainedHost, RetainedInput};

fn fixture(root: &std::path::Path, seed: u64) -> BoardDriver {
    let mut driver =
        BoardDriver::with_board(RetainedBoard::new(ChessState::initial(), seed).unwrap());
    driver.open_store(root.to_path_buf());
    driver
}
fn host(driver: &BoardDriver) -> RetainedHost {
    let scene = driver.board().unwrap().scene();
    RetainedHost::new(
        scene.clone(),
        viewer_options(scene, CameraPreset::Overhead, [800, 600]),
    )
    .unwrap()
}

#[test]
fn required_gpu_continue_after_three_moves_restores_exact_pixels() {
    let root = tempfile::tempdir().unwrap();
    let mut first = fixture(root.path(), 42);
    for uci in ["e2e4", "e7e5", "g1f3"] {
        first.play_uci(uci).unwrap();
    }
    let expected = first.board().unwrap().game().clone();
    let queue = first.queue();
    let inbox = first.captures();
    let mut run = host(&first)
        .windowless(&mut first)
        .expect("Metal/lavapipe required");
    let before = capture_frame(&mut run, &queue, &inbox).unwrap();
    run.shutdown().unwrap();
    drop(run);
    drop(first);
    let mut next = fixture(root.path(), 999);
    assert!(next.can_continue());
    next.show_title().unwrap();
    let inputs = next.inputs();
    let queue = next.queue();
    let inbox = next.captures();
    let mut run = host(&next).windowless(&mut next).unwrap();
    capture_frame(&mut run, &queue, &inbox).unwrap();
    inputs.borrow_mut().extend([
        RetainedInput::PointerMoved([204., 440.]),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ]);
    let after = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_eq!(
        before.pixels, after.pixels,
        "relaunch must restore the entire board and native status exactly"
    );
    run.shutdown().unwrap();
    drop(run);
    assert_eq!(next.board().unwrap().game(), &expected);
    assert_eq!(next.board().unwrap().position().seed, 42);
}

#[test]
fn required_gpu_resumed_computer_takes_its_turn_through_native_workready() {
    let root = tempfile::tempdir().unwrap();
    let mut first = fixture(root.path(), 42);
    first
        .set_computer(ComputerGame::new(Side::Ivory, 1, 42).unwrap())
        .unwrap();
    first.play_uci("e2e4").unwrap();
    let mut next = fixture(root.path(), 42);
    next.continue_game().unwrap();
    assert_eq!(
        (
            next.computer_game().unwrap().human(),
            next.computer_game().unwrap().level()
        ),
        (Side::Ivory, 1)
    );
    let queue = next.queue();
    let inbox = next.captures();
    let mut run = host(&next).windowless(&mut next).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        capture_frame(&mut run, &queue, &inbox).unwrap();
        let bytes = std::fs::read(root.path().join("slot-current.rds")).unwrap();
        if bytes.windows(4).any(|slice| slice == b"b8c6") {
            break;
        }
        assert!(
            std::time::Instant::now() < deadline,
            "resumed computer reply missing"
        );
    }
    run.shutdown().unwrap();
    drop(run);
    let mut expected = ChessState::initial();
    for uci in ["e2e4", "b8c6"] {
        expected.play(expected.parse_uci(uci).unwrap()).unwrap();
    }
    assert_eq!(next.board().unwrap().game(), &expected);
}
