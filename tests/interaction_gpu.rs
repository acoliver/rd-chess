//! Required Metal/lavapipe windowless play. No visible window or device skips.
#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, PieceKind, Side, Square,
    retained::{BoardDriver, InputInbox, RetainedBoard, capture_frame, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::{SceneView, Viewport, offscreen::RgbaImage},
    viewer::{MouseButton, RetainedHost, RetainedInput},
};

fn setup(game: ChessState) -> (BoardDriver, SceneView, RetainedHost) {
    let board = RetainedBoard::new(game, 342).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let view = SceneView::new(
        options.pose,
        options.camera,
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    let host = RetainedHost::new(board.scene().clone(), options).unwrap();
    (BoardDriver::with_board(board), view, host)
}

fn assert_markers(host: &RetainedHost, squares: &[&str]) {
    let actual: std::collections::BTreeSet<_> = host
        .objects()
        .filter(|(_, object)| object.mesh().raw() == 11)
        .map(|(_, object)| object.transform().position_millimeters)
        .collect();
    let expected = squares
        .iter()
        .map(|name| {
            let square = square(name);
            [
                i64::from(square.file) * 1000 - 3500,
                i64::from(square.rank) * 1000 - 3500,
                65,
            ]
        })
        .collect();
    assert_eq!(actual, expected);
}

fn click(inputs: &InputInbox, pixel: [f64; 2]) {
    inputs.borrow_mut().extend([
        RetainedInput::PointerMoved(pixel),
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
fn project(view: &SceneView, square: Square) -> [f64; 2] {
    let screen = view
        .resolved_view()
        .world_to_screen(glam::Vec3::new(
            f32::from(square.file) - 3.5,
            f32::from(square.rank) - 3.5,
            0.05,
        ))
        .unwrap();
    [f64::from(screen.x), f64::from(screen.y)]
}
fn square(text: &str) -> Square {
    Square::new(text.as_bytes()[0] - b'a', text.as_bytes()[1] - b'1').unwrap()
}
fn changed(left: &RgbaImage, right: &RgbaImage, rect: [u32; 4]) -> usize {
    let mut count = 0;
    for y in rect[1]..rect[3] {
        for x in rect[0]..rect[2] {
            let index = ((y * left.viewport.width + x) * 4) as usize;
            if left.pixels[index..index + 4] != right.pixels[index..index + 4] {
                count += 1;
            }
        }
    }
    count
}
fn save(image: &RgbaImage, name: &str) {
    let Some(directory) = std::env::var_os("CHESS3D_TEST_CAPTURE_DIR") else {
        return;
    };
    let path = std::path::Path::new(&directory).join(format!("{name}.png"));
    let file = std::fs::File::create(path).unwrap();
    let mut encoder = png::Encoder::new(file, image.viewport.width, image.viewport.height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .unwrap()
        .write_image_data(&image.pixels)
        .unwrap();
}

#[test]
fn required_gpu_click_moves_show_status_and_move_board_pixels() {
    let (mut driver, view, host) = setup(ChessState::initial());
    let inputs = driver.inputs();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host
        .windowless(&mut driver)
        .expect("GPU required, never skip");
    let initial = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&initial, "initial-status");
    assert_eq!(run.host().accepted_generation(), 1);
    assert_eq!(
        initial.pixels,
        capture_frame(&mut run, &queue, &inbox).unwrap().pixels
    );
    assert_eq!(
        run.host().accepted_generation(),
        1,
        "unchanged UI is not resubmitted"
    );
    click(&inputs, project(&view, square("e2")));
    let selected = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert!(changed(&initial, &selected, [100, 100, 700, 560]) > 20);
    assert_markers(run.host(), &["e2", "e3", "e4"]);
    click(&inputs, project(&view, square("e4")));
    let white_moved = capture_frame(&mut run, &queue, &inbox).unwrap();
    assert_markers(run.host(), &[]);
    assert_eq!(
        run.host().accepted_generation(),
        4,
        "selection plus one move batch and turn UI"
    );
    assert!(
        changed(&initial, &white_moved, [12, 8, 350, 60]) > 40,
        "side-to-move text changes above board"
    );
    assert!(
        changed(&initial, &white_moved, [350, 280, 470, 490]) > 50,
        "e2/e4 board pixels change"
    );
    click(&inputs, project(&view, square("e7")));
    capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, project(&view, square("e5")));
    let both_moved = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&both_moved, "two-click-moves-status");
    assert!(changed(&white_moved, &both_moved, [350, 120, 470, 310]) > 50);
    run.shutdown().unwrap();
    drop(run);
    assert_eq!(driver.board().unwrap().game().repetition_history().len(), 3);
    assert_eq!(
        driver.board().unwrap().game().piece_at(square("e5")),
        Some((Side::Onyx, PieceKind::Pawn))
    );
}

#[test]
fn required_gpu_promotion_dialog_consumes_click_and_shows_underpromotion() {
    let (mut driver, view, host) =
        setup(ChessState::from_fen("4k3/P6p/8/8/8/8/4b3/4K3 w - - 0 1").unwrap());
    let identities = driver.board().unwrap().identities().clone();
    let inputs = driver.inputs();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host
        .windowless(&mut driver)
        .expect("GPU required, never skip");
    let initial = capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, project(&view, square("a7")));
    let selected = capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, project(&view, square("a8")));
    let dialog = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&dialog, "promotion-dialog");
    assert!(
        changed(&selected, &dialog, [230, 180, 570, 490]) > 300,
        "four visible text choices"
    );
    assert_eq!(
        changed(&selected, &dialog, [12, 8, 350, 60]),
        0,
        "uncommitted promotion retains turn"
    );
    click(&inputs, [410., 468.]);
    let promoted = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&promoted, "promoted-knight-status");
    assert!(
        changed(&dialog, &promoted, [230, 180, 570, 490]) > 300,
        "dialog closes"
    );
    assert!(
        changed(&initial, &promoted, [12, 8, 350, 60]) > 40,
        "turn changes"
    );
    assert!(
        changed(&initial, &promoted, [160, 70, 250, 180]) > 50,
        "a7/a8 piece changes"
    );
    run.shutdown().unwrap();
    drop(run);
    assert_eq!(
        driver.board().unwrap().game().piece_at(square("a8")),
        Some((Side::Ivory, PieceKind::Knight))
    );
    assert_eq!(driver.board().unwrap().game().repetition_history().len(), 2);
    assert_eq!(driver.board().unwrap().identities(), &identities);
    assert_eq!(driver.selected(), None);
    assert!(!driver.promotion_pending());
}

#[test]
fn required_gpu_computer_replies_to_synthetic_e2e4() {
    let (mut driver, view, host) = setup(ChessState::initial());
    driver
        .set_computer(chess3d::retained::ComputerGame::new(Side::Ivory, 1, 42).unwrap())
        .unwrap();
    let knight = driver
        .board()
        .unwrap()
        .position()
        .pieces
        .into_iter()
        .find(|piece| piece.square == square("g8"))
        .unwrap();
    let id = driver.board().unwrap().identities()[&knight.id].body;
    let initial_object = driver.board().unwrap().objects()[&id];
    let inputs = driver.inputs();
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host
        .windowless(&mut driver)
        .expect("GPU required, never skip");
    let initial = capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, project(&view, square("e2")));
    capture_frame(&mut run, &queue, &inbox).unwrap();
    click(&inputs, project(&view, square("e4")));
    wait_for_reply(&mut run, id, initial_object);
    let replied = capture_frame(&mut run, &queue, &inbox).unwrap();
    save(&replied, "computer-level-1-reply");
    let object = run
        .host()
        .objects()
        .find(|(object_id, _)| *object_id == id)
        .unwrap()
        .1;
    assert_eq!(object.transform().position_millimeters, [1500, 1500, 50]);
    assert!(changed(&initial, &replied, [450, 60, 610, 280]) > 50);
    run.shutdown().unwrap();
    drop(run);
    let mut expected = ChessState::initial();
    for text in ["e2e4", "g8f6"] {
        expected.play(expected.parse_uci(text).unwrap()).unwrap();
    }
    assert_eq!(driver.board().unwrap().game(), &expected);
    assert_eq!(driver.status_text(), "White to move");
    assert_eq!(replied.pixels, reference_reply().pixels);
}

fn reference_reply() -> RgbaImage {
    let (mut driver, _, host) = setup(ChessState::initial());
    for text in ["e2e4", "g8f6"] {
        driver.play_uci(text).unwrap();
    }
    let queue = driver.queue();
    let inbox = driver.captures();
    let mut run = host.windowless(&mut driver).unwrap();
    let result = capture_frame(&mut run, &queue, &inbox).unwrap();
    run.shutdown().unwrap();
    result
}

fn wait_for_reply(
    run: &mut worldrunner::scene::viewer::WindowlessRetained<'_>,
    id: worldrunner::scene::viewer::ObjectId,
    initial_object: worldrunner::scene::plan::SceneObject,
) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
    loop {
        assert!(
            std::time::Instant::now() < deadline,
            "computer did not publish its reply"
        );
        run.frame().unwrap();
        run.poll().unwrap();
        if run
            .host()
            .objects()
            .any(|(object_id, object)| object_id == id && object != initial_object)
        {
            break;
        }
        std::thread::yield_now();
    }
}
