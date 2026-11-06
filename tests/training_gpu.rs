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
            (16.0..=23.0).contains(&(f64::from(x) - center[0]).hypot(f64::from(y) - center[1]))
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
fn assert_uniform_panel(image: &RgbaImage) {
    // Sample gaps between rows over the board, as well as above it.
    let samples = [
        [150, 120],
        [150, 180],
        [300, 180],
        [500, 180],
        [200, 270],
        [400, 270],
        [600, 270],
        [200, 366],
        [400, 366],
        [600, 366],
    ];
    let pixel = |[x, y]: [u32; 2]| {
        let offset = ((y * image.viewport.width + x) * 4) as usize;
        &image.pixels[offset..offset + 4]
    };
    let reference = pixel(samples[0]);
    for point in samples {
        assert!(
            pixel(point)
                .iter()
                .zip(reference)
                .all(|(&actual, &expected)| actual.abs_diff(expected) <= 1),
            "panel at {point:?}: {:?}, reference {reference:?}",
            pixel(point)
        );
    }
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
    assert_uniform_panel(&list);
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
        "cyan ring pixels must surround authored e3, independently of driver state"
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

#[test]
fn required_gpu_authored_arrow_pixels_follow_only_the_projected_board_line() {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let host = RetainedHost::new(board.scene().clone(), options).unwrap();
    let mut driver = BoardDriver::with_board(board);
    driver.show_title().unwrap();
    let (inputs, queue, inbox) = (driver.inputs(), driver.queue(), driver.captures());
    let mut run = host.windowless(&mut driver).unwrap();
    capture_frame(&mut run, &queue, &inbox).unwrap();
    for name in ["home", "arrowdown", "arrowdown", "enter", "enter"] {
        key(&inputs, name);
    }
    capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, [240., 565.]);
    capture_frame(&mut run, &queue, &inbox).unwrap();
    for point in training_move_pixels("e2e4", [800, 600]).unwrap() {
        click(&inputs, point);
        capture_frame(&mut run, &queue, &inbox).unwrap();
    }
    click(&inputs, [240., 565.]);
    let arrow = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&arrow, "lesson-authored-arrow.png");
    let line = training_move_pixels("g1f3", [800, 600]).unwrap();
    let (along, elsewhere) = arrow_pixels(&arrow, line);
    assert!(along > 120, "authored cyan arrow on board line: {along}");
    assert_eq!(
        elsewhere, 0,
        "no cyan arrow pixels outside its board corridor"
    );
    assert_eq!(
        cyan_near(&arrow, line[0]),
        0,
        "the knight occludes the board arrow at its source"
    );
    click(&inputs, [240., 565.]);
    let cleared = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_eq!(
        arrow_pixels(&cleared, line),
        (0, 0),
        "Next clears the previous step's arrow"
    );
    run.shutdown().unwrap();
}
fn arrow_pixels(image: &RgbaImage, [from, to]: [[f64; 2]; 2]) -> (usize, usize) {
    let delta = [to[0] - from[0], to[1] - from[1]];
    let length = delta[0].powi(2) + delta[1].powi(2);
    image
        .pixels
        .chunks_exact(4)
        .enumerate()
        .filter(|(_, p)| authored_cyan(p))
        .fold((0, 0), |(along, elsewhere), (index, _)| {
            let index = u32::try_from(index).unwrap();
            let p = [
                f64::from(index % image.viewport.width),
                f64::from(index / image.viewport.width),
            ];
            let t = (((p[0] - from[0]) * delta[0] + (p[1] - from[1]) * delta[1]) / length)
                .clamp(0., 1.);
            let distance = (p[0] - from[0] - t * delta[0]).hypot(p[1] - from[1] - t * delta[1]);
            if distance <= 14. {
                (along + 1, elsewhere)
            } else {
                (along, elsewhere + 1)
            }
        })
}
fn authored_cyan(pixel: &[u8]) -> bool {
    // Linear authored RGBA [0, .7, .8, 1] in the sRGB capture attachment.
    pixel[0] <= 1 && pixel[1].abs_diff(218) <= 1 && pixel[2].abs_diff(231) <= 1 && pixel[3] == 255
}
fn cyan_near(image: &RgbaImage, centre: [f64; 2]) -> usize {
    image
        .pixels
        .chunks_exact(4)
        .enumerate()
        .filter(|(index, pixel)| {
            let index = u32::try_from(*index).unwrap();
            let x = f64::from(index % image.viewport.width);
            let y = f64::from(index / image.viewport.width);
            (x - centre[0]).hypot(y - centre[1]) < 5. && authored_cyan(pixel)
        })
        .count()
}
