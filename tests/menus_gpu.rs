//! Required windowless native-menu composition on the same retained board.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset,
    retained::{BoardDriver, InputInbox, RetainedBoard, capture_frame, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::offscreen::RgbaImage,
    viewer::{KeyCode, LogicalKey, RetainedHost, RetainedInput},
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
fn changed(left: &RgbaImage, right: &RgbaImage, rect: [u32; 4]) -> usize {
    (rect[1]..rect[3])
        .flat_map(|y| (rect[0]..rect[2]).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let at = ((y * left.viewport.width + x) * 4) as usize;
            left.pixels[at..at + 4] != right.pixels[at..at + 4]
        })
        .count()
}
fn focused_tint(image: &RgbaImage, rect: [u32; 4]) -> usize {
    (rect[1]..rect[3])
        .flat_map(|y| (rect[0]..rect[2]).map(move |x| (x, y)))
        .filter(|&(x, y)| {
            let at = ((y * image.viewport.width + x) * 4) as usize;
            let rgb = &image.pixels[at..at + 3];
            rgb[0] > 160 && rgb[0] > rgb[1] && u16::from(rgb[1]) > u16::from(rgb[2]) + 35
        })
        .count()
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

#[test]
fn required_gpu_title_and_pause_have_visible_native_focus_and_stable_pixels() {
    let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
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
    let title = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&title, "title-menu.png");
    assert_eq!(
        title.pixels,
        capture_frame(&mut run, &queue, &inbox).unwrap().pixels
    );
    assert!(
        focused_tint(&title, [230, 130, 570, 170]) > 60,
        "first title item is tinted"
    );
    key(&inputs, "enter");
    let board = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert!(changed(&title, &board, [90, 120, 710, 480]) > 10_000);
    assert_eq!(
        changed(&title, &board, [0, 150, 70, 550]),
        0,
        "board remains unchanged outside menu"
    );
    key(&inputs, "escape");
    let pause = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&pause, "in-game-menu.png");
    assert!(changed(&pause, &board, [90, 120, 710, 480]) > 10_000);
    assert!(
        focused_tint(&pause, [230, 185, 570, 230]) > 60,
        "Resume is tinted"
    );
    assert_eq!(
        pause.pixels,
        capture_frame(&mut run, &queue, &inbox).unwrap().pixels
    );
    key(&inputs, "arrowdown");
    let navigated = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert!(changed(&pause, &navigated, [230, 185, 570, 340]) > 100);
    assert!(
        focused_tint(&navigated, [230, 290, 570, 335]) > 60,
        "navigation skips disabled Undo and tints Restart"
    );
    key(&inputs, "escape");
    assert_eq!(
        board.pixels,
        capture_frame(&mut run, &queue, &inbox).unwrap().pixels
    );
    run.shutdown().unwrap();
}
