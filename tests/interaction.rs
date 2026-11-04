#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, PieceKind, Side, Square,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::{SceneView, Viewport},
    viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput},
};

fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').unwrap()
}
fn fixture(fen: &str) -> (BoardDriver, SceneView) {
    let board = RetainedBoard::new(ChessState::from_fen(fen).unwrap(), 342).unwrap();
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
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    (driver, view)
}
fn pixel(view: &SceneView, name: &str) -> [f64; 2] {
    let location = square(name);
    let point = glam::Vec3::new(
        f32::from(location.file) - 3.5,
        f32::from(location.rank) - 3.5,
        0.05,
    );
    let screen = view.resolved_view().world_to_screen(point).unwrap();
    [f64::from(screen.x), f64::from(screen.y)]
}
fn pointer_button(driver: &mut BoardDriver, view: &SceneView, pressed: bool) -> bool {
    driver
        .handle_input(
            &RetainedInput::Button {
                button: MouseButton::Left,
                pressed,
            },
            Some(view),
            None,
        )
        .unwrap()
}
fn click_pixel(driver: &mut BoardDriver, view: &SceneView, pixel: [f64; 2]) -> bool {
    driver
        .handle_input(&RetainedInput::PointerMoved(pixel), Some(view), None)
        .unwrap();
    pointer_button(driver, view, true);
    pointer_button(driver, view, false)
}

fn click(driver: &mut BoardDriver, view: &SceneView, name: &str) {
    click_pixel(driver, view, pixel(view, name));
}
fn key(driver: &mut BoardDriver, view: &SceneView, name: &str) {
    driver
        .handle_input(
            &RetainedInput::Key {
                key: name.into(),
                logical: LogicalKey::Character(name.into()),
                physical: KeyCode::KeyU.into(),
                pressed: true,
                repeat: false,
            },
            Some(view),
            None,
        )
        .unwrap();
}

#[test]
fn select_destinations_deselect_and_illegal_noop() {
    let (mut driver, view) = fixture(chess3d::rules::INITIAL_FEN);
    click(&mut driver, &view, "e2");
    assert_eq!(driver.selected(), Some(square("e2")));
    let expected: std::collections::BTreeSet<_> = driver
        .board()
        .unwrap()
        .game()
        .legal_moves()
        .into_iter()
        .filter(|m| m.from() == square("e2"))
        .map(chess3d::rules::LegalMove::to)
        .collect();
    assert_eq!(driver.destinations(), expected);
    let game = driver.board().unwrap().game().clone();
    click(&mut driver, &view, "e5");
    assert_eq!(driver.selected(), Some(square("e2")));
    assert_eq!(driver.board().unwrap().game(), &game);
    assert!(driver.queue().borrow().is_empty());
    click(&mut driver, &view, "e2");
    assert_eq!(driver.selected(), None);
    click(&mut driver, &view, "e7");
    assert_eq!(driver.selected(), None);
    click(&mut driver, &view, "d2");
    click_pixel(&mut driver, &view, [0., 0.]);
    assert_eq!(driver.selected(), None);
}

#[test]
fn click_commits_one_move_one_batch_and_undo_restart_keep_identities() {
    let (mut driver, view) = fixture(chess3d::rules::INITIAL_FEN);
    let initial = driver.board().unwrap().game().clone();
    let ids = driver.board().unwrap().identities().clone();
    click(&mut driver, &view, "e2");
    click(&mut driver, &view, "e4");
    assert_eq!(driver.queue().borrow().len(), 1);
    assert_eq!(driver.board().unwrap().game().repetition_history().len(), 2);
    assert_eq!(driver.selected(), None);
    key(&mut driver, &view, "u");
    assert_eq!(driver.board().unwrap().game(), &initial);
    assert_eq!(driver.queue().borrow().len(), 2);
    driver.queue().borrow_mut().clear();
    click(&mut driver, &view, "d2");
    click(&mut driver, &view, "d4");
    key(&mut driver, &view, "r");
    assert_eq!(driver.board().unwrap().game(), &initial);
    assert_eq!(driver.board().unwrap().identities(), &ids);
    assert_eq!(driver.queue().borrow().len(), 2);
}

#[test]
fn drag_orbits_without_moving_and_focus_loss_cancels_click() {
    let (mut driver, view) = fixture(chess3d::rules::INITIAL_FEN);
    let start = pixel(&view, "e2");
    driver
        .handle_input(&RetainedInput::PointerMoved(start), Some(&view), None)
        .unwrap();
    pointer_button(&mut driver, &view, true);
    driver
        .handle_input(
            &RetainedInput::PointerMoved([start[0] + 30., start[1] + 20.]),
            Some(&view),
            None,
        )
        .unwrap();
    pointer_button(&mut driver, &view, false);
    assert_ne!(driver.camera_pose().unwrap(), CameraPreset::Overhead.pose());
    assert!(driver.queue().borrow().is_empty());
    assert_eq!(driver.selected(), None);
    driver
        .handle_input(&RetainedInput::PointerMoved(start), Some(&view), None)
        .unwrap();
    pointer_button(&mut driver, &view, true);
    driver
        .handle_input(&RetainedInput::PointerCancelled, Some(&view), None)
        .unwrap();
    pointer_button(&mut driver, &view, false);
    assert_eq!(driver.selected(), None);
}

#[test]
fn promotion_routes_knight_choice_and_consumes_board_click() {
    let (mut driver, view) = fixture("4k3/P7/8/8/8/8/4b3/4K3 w - - 0 1");
    click(&mut driver, &view, "a7");
    click(&mut driver, &view, "a8");
    assert!(driver.promotion_pending());
    assert!(driver.queue().borrow().is_empty());
    assert_eq!(driver.board().unwrap().game().side_to_move(), Side::Ivory);
    assert!(click_pixel(&mut driver, &view, driver_choice_pixel(3)));
    assert!(!driver.promotion_pending());
    assert_eq!(
        driver.board().unwrap().game().piece_at(square("a8")),
        Some((Side::Ivory, PieceKind::Knight))
    );
    assert_eq!(driver.queue().borrow().len(), 1);
    assert_eq!(driver.board().unwrap().game().repetition_history().len(), 2);
    assert_eq!(driver.selected(), None);
}
fn driver_choice_pixel(index: u8) -> [f64; 2] {
    [410., 300. - (-42. + f64::from(index) * -42.)]
}

#[test]
fn escape_undo_and_restart_close_promotion_without_committing() {
    for command in ["escape", "backspace", "r"] {
        let (mut driver, view) = fixture("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let initial = driver.board().unwrap().game().clone();
        click(&mut driver, &view, "a7");
        click(&mut driver, &view, "a8");
        key(&mut driver, &view, command);
        assert!(!driver.promotion_pending());
        assert_eq!(driver.board().unwrap().game(), &initial);
        assert_eq!(driver.selected(), None);
        assert!(!driver.exit_requested());
    }
}

#[test]
fn resident_marker_objects_match_rules_and_status_does_not_resubmit() {
    use worldrunner::scene::viewer::{RetainedDriver, RetainedHost, RetainedTick};
    let (mut driver, view) = fixture(chess3d::rules::INITIAL_FEN);
    let board = driver.board().unwrap();
    let meshes = board.scene().geometry().meshes().as_ptr();
    let textures = board.scene().resources().textures().as_ptr();
    let mut host = RetainedHost::new(
        board.scene().clone(),
        viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]),
    )
    .unwrap();
    let tick = RetainedTick {
        frame: 0,
        elapsed: std::time::Duration::ZERO,
        dt: std::time::Duration::ZERO,
    };
    click(&mut driver, &view, "e2");
    let mut expected = driver.destinations();
    expected.insert(square("e2"));
    driver.update(&mut host, &tick).unwrap();
    let actual: std::collections::BTreeSet<_> = host
        .objects()
        .filter(|(_, object)| object.mesh().raw() == 11)
        .map(|(_, object)| {
            let point = object.transform().position_millimeters;
            Square::new(
                u8::try_from((point[0] + 3500) / 1000).unwrap(),
                u8::try_from((point[1] + 3500) / 1000).unwrap(),
            )
            .unwrap()
        })
        .collect();
    assert_eq!(actual, expected);
    driver.update(&mut host, &tick).unwrap();
    let generation = host.accepted_generation();
    driver.update(&mut host, &tick).unwrap();
    assert_eq!(host.accepted_generation(), generation);
    assert_eq!(
        driver.board().unwrap().scene().geometry().meshes().as_ptr(),
        meshes
    );
    assert_eq!(
        driver
            .board()
            .unwrap()
            .scene()
            .resources()
            .textures()
            .as_ptr(),
        textures
    );
}

#[test]
fn terminal_positions_and_unpublished_moves_reject_board_input() {
    let (mut driver, view) = fixture("4k3/8/8/8/8/8/8/R3K3 w - - 150 1");
    click(&mut driver, &view, "a1");
    assert_eq!(driver.selected(), None);
    assert!(driver.play_uci("a1a2").is_err());
    let (mut driver, view) = fixture(chess3d::rules::INITIAL_FEN);
    click(&mut driver, &view, "e2");
    click(&mut driver, &view, "e4");
    click(&mut driver, &view, "e7");
    assert_eq!(driver.selected(), None);
    assert_eq!(driver.queue().borrow().len(), 1);
}

#[test]
fn status_names_turn_check_mate_stalemate_and_draw_rules() {
    for (fen, expected) in [
        (chess3d::rules::INITIAL_FEN, "White to move"),
        ("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1", "check"),
        ("7k/6Q1/5K2/8/8/8/8/8 b - - 0 1", "Checkmate: White wins"),
        ("7k/5Q2/5K2/8/8/8/8/8 b - - 0 1", "stalemate"),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 1", "insufficient material"),
        ("4k3/8/8/8/8/8/8/R3K3 w - - 150 1", "seventy-five-move rule"),
        (
            "4k3/8/8/8/8/8/8/R3K3 w - - 100 1",
            "fifty-move rule claim available",
        ),
    ] {
        let (driver, _) = fixture(fen);
        assert!(
            driver.status_text().contains(expected),
            "{}",
            driver.status_text()
        );
    }
    let (mut driver, _) = fixture(chess3d::rules::INITIAL_FEN);
    for _ in 0..2 {
        for uci in ["g1f3", "g8f6", "f3g1", "f6g8"] {
            driver.play_uci(uci).unwrap();
        }
    }
    assert!(
        driver
            .status_text()
            .contains("threefold repetition claim available")
    );
    for _ in 0..2 {
        for uci in ["g1f3", "g8f6", "f3g1", "f6g8"] {
            driver.play_uci(uci).unwrap();
        }
    }
    assert!(driver.status_text().contains("fivefold repetition"));
}
