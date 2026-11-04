//! Required windowless native teaching UI and authored-square pixel checks.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{
        BoardDriver, InputInbox, RetainedBoard, capture_frame, training_move_pixels, viewer_options,
    },
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::offscreen::RgbaImage,
    viewer::{KeyCode, LogicalKey, MouseButton, RetainedHost, RetainedInput},
};

fn key(inputs: &InputInbox, key: &str) {
    inputs
        .borrow_mut()
        .extend([true, false].map(|pressed| RetainedInput::Key {
            key: key.into(),
            logical: LogicalKey::Character(key.into()),
            physical: KeyCode::Enter.into(),
            pressed,
            repeat: false,
        }));
}
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
fn save(image: &RgbaImage, name: &str) {
    if let Some(directory) = std::env::var_os("CHESS3D_TEST_CAPTURE_DIR") {
        let file = std::fs::File::create(std::path::Path::new(&directory).join(name)).unwrap();
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
fn tint(image: &RgbaImage, center: [f64; 2]) -> usize {
    (0..image.viewport.height)
        .flat_map(|y| (0..image.viewport.width).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            (f64::from(x) - center[0]).abs() < 12. && (f64::from(y) - center[1]).abs() < 12.
        })
        .filter(|&(x, y)| {
            let at = ((y * image.viewport.width + x) * 4) as usize;
            let [r, g, b] =
                [image.pixels[at], image.pixels[at + 1], image.pixels[at + 2]].map(u16::from);
            g > r + 40 && b > r + 40
        })
        .count()
}
fn focus_pixels(image: &RgbaImage) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .filter(|p| p[0] > 160 && p[0] > p[1] && u16::from(p[1]) > u16::from(p[2]) + 35)
        .count()
}

#[test]
fn required_gpu_lessons_list_and_task_show_authored_highlight_pixels() {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let host = RetainedHost::new(board.scene().clone(), options).unwrap();
    let mut driver = BoardDriver::with_board(board);
    driver.show_title().unwrap();
    let inputs = driver.inputs();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host
        .windowless(&mut driver)
        .expect("Metal/lavapipe GPU required, no skips");
    capture_frame(&mut run, &queue, &inbox).unwrap();
    for name in ["home", "arrowdown", "arrowdown", "enter"] {
        key(&inputs, name);
    }
    let list = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&list, "lessons-list.png");
    assert!(
        focus_pixels(&list) > 60,
        "unlocked lesson has visible native focus"
    );
    assert_eq!(
        list.pixels,
        capture_frame(&mut run, &queue, &inbox).unwrap().pixels
    );
    key(&inputs, "enter");
    let explanation = capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, [240., 565.]);
    let task = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&task, "lesson-task-highlight.png");
    let [e3, d3] = training_move_pixels("e3d3", [800, 600]).unwrap();
    assert!(
        tint(&task, e3) > 200,
        "cyan pixels must cover authored e3, independently of driver state"
    );
    assert!(
        tint(&explanation, e3) < 20,
        "same square was unmarked in explanation"
    );
    assert!(
        tint(&task, d3) < 20,
        "adjacent unauthored d3 must not have cyan highlight"
    );
    assert_eq!(
        task.pixels,
        capture_frame(&mut run, &queue, &inbox).unwrap().pixels
    );
    run.shutdown().unwrap();
}
