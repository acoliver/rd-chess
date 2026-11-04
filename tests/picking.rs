#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Square,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::{SceneView, Viewport},
    viewer::{MouseButton, RetainedInput, RetainedPickHit},
};

fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').unwrap()
}
fn fixture(preset: CameraPreset) -> (BoardDriver, SceneView) {
    let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let options = viewer_options(board.scene(), preset, [800, 600]);
    let view = SceneView::new(
        options.pose,
        options.camera,
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    (driver, view)
}
fn project(view: &SceneView, point: glam::Vec3) -> [f64; 2] {
    let screen = view.resolved_view().world_to_screen(point).unwrap();
    [f64::from(screen.x), f64::from(screen.y)]
}
fn click(driver: &mut BoardDriver, view: &SceneView, point: [f64; 2]) {
    let board = driver.board().unwrap();
    let ray = view
        .resolved_view()
        .screen_ray(glam::DVec2::from_array(point).as_vec2())
        .unwrap();
    let hit = board
        .scene()
        .pick(
            ray,
            board.objects().iter().map(|(&id, &object)| (id, object)),
            |id| id.0 == 0 || id.0 >= 4,
        )
        .map(|hit| RetainedPickHit { generation: 0, hit });
    for input in [
        RetainedInput::PointerMoved(point),
        button(true),
        button(false),
    ] {
        driver.handle_input(&input, Some(view), hit).unwrap();
    }
}
fn button(pressed: bool) -> RetainedInput {
    RetainedInput::Button {
        button: MouseButton::Left,
        pressed,
    }
}

#[test]
fn hero_tall_piece_top_selects_its_square_not_the_board_plane_square() {
    for (name, height) in [("e1", 1.6), ("d1", 1.44)] {
        let (mut driver, view) = fixture(CameraPreset::Hero);
        let square = square(name);
        let point = project(
            &view,
            glam::Vec3::new(
                f32::from(square.file) - 3.5,
                f32::from(square.rank) - 3.5,
                height,
            ),
        );
        let plane = view
            .resolved_view()
            .screen_ray(glam::DVec2::from_array(point).as_vec2())
            .unwrap()
            .intersect_plane(glam::Vec3::new(0., 0., 0.05), glam::Vec3::Z)
            .unwrap();
        assert!(
            (plane.x - (f32::from(square.file) - 3.5)).abs() > 0.5
                || (plane.y - (f32::from(square.rank) - 3.5)).abs() > 0.5,
            "fixture must distinguish silhouette from board-plane picking"
        );
        click(&mut driver, &view, point);
        assert_eq!(driver.selected(), Some(square), "{name} top");
    }
}

#[test]
fn accepted_unpresented_move_blocks_clicks_even_without_frame_statistics() {
    use worldrunner::scene::viewer::{RetainedDriver, RetainedHost, RetainedTick};
    let (mut driver, view) = fixture(CameraPreset::Overhead);
    let board = driver.board().unwrap();
    let mut host = RetainedHost::new(
        board.scene().clone(),
        viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]),
    )
    .unwrap();
    driver.play_uci("e2e4").unwrap();
    let tick = RetainedTick {
        frame: 0,
        elapsed: std::time::Duration::ZERO,
        dt: std::time::Duration::ZERO,
    };
    driver.update(&mut host, &tick).unwrap();
    assert!(driver.queue().borrow().is_empty());
    assert_eq!(host.accepted_generation(), 1);
    driver.update(&mut host, &tick).unwrap();
    driver.update(&mut host, &tick).unwrap();
    let game = driver.board().unwrap().game().clone();
    click(
        &mut driver,
        &view,
        project(&view, glam::Vec3::new(0.5, 2.5, 0.05)),
    );
    assert_eq!(driver.selected(), None);
    assert_eq!(driver.board().unwrap().game(), &game);
    assert!(driver.queue().borrow().is_empty());
}

#[test]
fn mismatched_hit_generation_is_noop_instead_of_plane_fallback() {
    let (mut driver, view) = fixture(CameraPreset::Overhead);
    click(
        &mut driver,
        &view,
        project(&view, glam::Vec3::new(0.5, -2.5, 0.05)),
    );
    assert_eq!(driver.selected(), Some(square("e2")));
    let point = project(&view, glam::Vec3::new(-0.5, -2.5, 0.05));
    let board = driver.board().unwrap();
    let ray = view
        .resolved_view()
        .screen_ray(glam::DVec2::from_array(point).as_vec2())
        .unwrap();
    let hit = board
        .scene()
        .pick(
            ray,
            board.objects().iter().map(|(&id, &object)| (id, object)),
            |_| true,
        )
        .unwrap();
    for input in [
        RetainedInput::PointerMoved(point),
        button(true),
        button(false),
    ] {
        driver
            .handle_input(
                &input,
                Some(&view),
                Some(RetainedPickHit { generation: 1, hit }),
            )
            .unwrap();
    }
    assert_eq!(driver.selected(), Some(square("e2")));
    assert!(driver.queue().borrow().is_empty());
}
