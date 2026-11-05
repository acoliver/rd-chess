//! Required windowless review overlay and independent recorded-position capture.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{BoardDriver, InputInbox, RetainedBoard, capture_frame, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::offscreen::RgbaImage,
    viewer::{MouseButton, RetainedHost, RetainedInput},
};

fn click(inputs: &InputInbox, point: [f64; 2]) {
    inputs.borrow_mut().extend([
        RetainedInput::PointerMoved(point),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ]);
}
fn crop(image: &RgbaImage, rect: [u32; 4]) -> Vec<u8> {
    (rect[1]..rect[3])
        .flat_map(|y| {
            let first = ((y * image.viewport.width + rect[0]) * 4) as usize;
            let end = ((y * image.viewport.width + rect[2]) * 4) as usize;
            image.pixels[first..end].to_vec()
        })
        .collect()
}
fn save(image: &RgbaImage) {
    if let Some(directory) = std::env::var_os("CHESS3D_TEST_CAPTURE_DIR") {
        let file =
            std::fs::File::create(std::path::Path::new(&directory).join("review-screen.png"))
                .unwrap();
        let mut encoder = png::Encoder::new(file, image.viewport.width, image.viewport.height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&image.pixels)
            .unwrap();
    }
}
fn board() -> RetainedBoard {
    RetainedBoard::new(
        ChessState::from_fen("7k/5Q2/5K2/8/8/8/8/8 w - - 0 1").unwrap(),
        42,
    )
    .unwrap()
}
#[test]
fn required_gpu_review_replay_matches_independent_initial_board() {
    let reference = board();
    let mut options = viewer_options(reference.scene(), CameraPreset::Overhead, [800, 600]);
    let ordinary = options.clone();
    options.pose = worldrunner::scene::gpu::CameraPose {
        eye_millimeters: [4_000, -2_000, 17_000],
        target_millimeters: [4_000, 0, 0],
    };
    let host = RetainedHost::new(reference.scene().clone(), options.clone()).unwrap();
    let mut reference = BoardDriver::with_board(reference);
    let queue = reference.queue();
    let inbox = reference.captures();
    let mut run = host
        .windowless(&mut reference)
        .expect("Metal/lavapipe required");
    let expected = capture_frame(&mut run, &queue, &inbox).unwrap();
    run.shutdown().unwrap();

    let board = board();
    let host = RetainedHost::new(board.scene().clone(), ordinary).unwrap();
    let mut driver = BoardDriver::with_board(board);
    driver.play_uci("f7g7").unwrap();
    let inputs = driver.inputs();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host
        .windowless(&mut driver)
        .expect("Metal/lavapipe required");
    let result = capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, [400., 440.]);
    let review = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&review);
    assert_ne!(
        crop(&result, [492, 100, 790, 570]),
        crop(&review, [492, 100, 790, 570]),
        "native review panel replaces result panel"
    );
    click(&inputs, [526., 140.]);
    let start = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_eq!(
        crop(&start, [0, 130, 480, 590]),
        crop(&expected, [0, 130, 480, 590]),
        "replay start has independently constructed initial board pixels outside the overlay"
    );
    assert!(
        crop(&review, [492, 120, 780, 250])
            .chunks_exact(4)
            .filter(|p| p[0] > 150 && p[1] > 150 && p[2] > 150)
            .count()
            > 100,
        "review controls and moves contain visible native text"
    );
    run.shutdown().unwrap();
}
