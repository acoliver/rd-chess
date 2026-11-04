//! Required windowless retained acceptance. Missing adapters fail, never skip.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{BoardDriver, RetainedBoard, capture_frame, viewer_options},
    rules::ChessState,
};
use std::collections::BTreeMap;
use worldrunner::scene::viewer::RetainedHost;

const SEQUENCE: &str = "e2e4 e7e5 g1f3 b8c6 f1c4 g8f6 d2d3 f8c5 e1g1 d7d6 c1g5 h7h6 g5f6";

#[test]
fn required_gpu_capture_moves_capture_castle_and_exact_undo() {
    let mut board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let identities = board.identities().clone();
    let initial_objects = board.objects().clone();
    let options = viewer_options(board.scene(), CameraPreset::Hero, [257, 193]);
    let host = RetainedHost::with_preparation_limit(board.scene().clone(), options, 1).unwrap();
    let mut driver = BoardDriver::default();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host
        .windowless(&mut driver)
        .expect("retained GPU acceptance requires a device, never skips");
    let initial = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_eq!(initial.pixels.len(), 257 * 193 * 4);
    assert!(initial.pixels.chunks_exact(4).all(|p| p[3] == 255));
    assert!(initial.pixels.chunks_exact(4).any(|p| p[0] > 150));
    assert!(initial.pixels.chunks_exact(4).any(|p| p[0] < 80));
    for uci in SEQUENCE.split_whitespace() {
        queue.borrow_mut().push_back(board.play_uci(uci).unwrap());
    }
    let moved = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_ne!(initial.pixels, moved.pixels);
    assert_eq!(
        run.host().accepted_generation(),
        13,
        "one generation per move, including castle/capture"
    );
    assert_eq!(
        run.host().objects().collect::<BTreeMap<_, _>>(),
        *board.objects()
    );
    for _ in SEQUENCE.split_whitespace() {
        queue.borrow_mut().push_back(board.undo().unwrap());
    }
    let restored = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_eq!(run.host().accepted_generation(), 26);
    assert_eq!(
        initial.pixels, restored.pixels,
        "same-device exact initial image after undo"
    );
    assert_eq!(
        run.host().objects().collect::<BTreeMap<_, _>>(),
        initial_objects
    );
    assert_eq!(board.identities(), &identities);
    run.shutdown().unwrap();
}

fn assert_cli_capture(expected: &[u8]) {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("moves.png");
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_chess3d"))
        .args([
            "--moves",
            SEQUENCE,
            "--width",
            "257",
            "--height",
            "193",
            "--capture",
        ])
        .arg(&path)
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let decoder = png::Decoder::new(std::io::BufReader::new(std::fs::File::open(&path).unwrap()));
    let mut reader = decoder.read_info().unwrap();
    let mut pixels = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut pixels).unwrap();
    assert_eq!((info.width, info.height), (257, 193));
    assert_eq!(&pixels[..info.buffer_size()], expected);
    let original = std::fs::read(&path).unwrap();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_chess3d"))
        .args(["--moves", "e2e5", "--capture"])
        .arg(&path)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("invalid UCI move 'e2e5'"));
    assert_eq!(std::fs::read(path).unwrap(), original);
}

#[test]
fn playable_driver_and_cli_have_exact_matching_status_and_board_pixels() {
    let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Hero, [257, 193]);
    let host = RetainedHost::with_preparation_limit(board.scene().clone(), options, 1).unwrap();
    let mut driver = BoardDriver::with_board(board);
    for uci in SEQUENCE.split_whitespace() {
        driver.play_uci(uci).unwrap();
    }
    let queue = driver.queue();
    let inbox = driver.captures();
    assert_eq!(queue.borrow().len(), 13, "one object batch per rules move");
    let mut run = host
        .windowless(&mut driver)
        .expect("GPU required, never skips");
    let moved = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_eq!(
        run.host().accepted_generation(),
        14,
        "thirteen object batches plus status UI"
    );
    assert_cli_capture(&moved.pixels);
    run.shutdown().unwrap();
}
