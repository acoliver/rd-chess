#[test]
fn draw_decline_and_resignation_cancel_leave_the_game_playable() {
    let mut driver = fixture(crate::rules::INITIAL_FEN);
    let before = driver.board().unwrap().game().clone();
    driver
        .menu_action(super::menus::MenuAction::Resign)
        .unwrap();
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), super::Screen::InGameMenu);
    assert!(driver.menus.result.is_none());
    driver
        .menu_action(super::menus::MenuAction::OfferDraw)
        .unwrap();
    snapshot(&mut driver);
    key(&mut driver, "escape");
    let update = snapshot(&mut driver);
    assert_eq!(
        update.announcements[0].text,
        "Black declined the draw offer"
    );
    assert_eq!(driver.screen(), super::Screen::Play);
    assert_eq!(driver.board().unwrap().game(), &before);
    driver
        .set_computer(ComputerGame::new(Side::Ivory, 1, 342).unwrap())
        .unwrap();
    driver
        .menu_action(super::menus::MenuAction::OfferDraw)
        .unwrap();
    assert_eq!(
        snapshot(&mut driver).announcements[0].text,
        "Computer declined the draw offer"
    );
    assert_eq!(driver.board().unwrap().game(), &before);
}

#[test]
fn focus_and_activation_follow_shared_turn_and_presentation_rules() {
    let mut driver = fixture(crate::rules::INITIAL_FEN);
    driver
        .set_computer(ComputerGame::new(Side::Onyx, 1, 342).unwrap())
        .unwrap();
    let update = snapshot(&mut driver);
    let square = update
        .changed
        .iter()
        .find(|n| n.label == "e2, white pawn")
        .unwrap()
        .id;
    driver
        .accessibility_input(UiAccessRequest {
            target: square,
            action: UiAccessAction::Focus,
        })
        .unwrap();
    assert_eq!(driver.semantic_update().unwrap().unwrap().focus, square);
    press(&mut driver, square);
    assert!(driver.selected().is_none());
    let mut driver = fixture(crate::rules::INITIAL_FEN);
    driver.play_uci("e2e4").unwrap();
    let update = snapshot(&mut driver);
    let square = update
        .changed
        .iter()
        .find(|n| n.label == "e7, black pawn")
        .unwrap()
        .id;
    press(&mut driver, square);
    assert!(driver.selected().is_none());
    driver.queue().borrow_mut().clear();
    press(&mut driver, square);
    assert_eq!(
        driver.selected(),
        Some(super::accessibility_tests::square("e7"))
    );
}

#[test]
fn menu_controls_and_lists_have_names() {
    let mut driver = fixture(crate::rules::INITIAL_FEN);
    for screen in [
        super::Screen::Title,
        super::Screen::ComputerSetup,
        super::Screen::Settings,
        super::Screen::Slots,
        super::Screen::Lessons,
        super::Screen::Puzzles,
    ] {
        driver.transition(screen).unwrap();
        let update = snapshot(&mut driver);
        for node in update.changed.iter().filter(|node| {
            !node.actions.is_empty() || matches!(node.role, UiRole::Dialog | UiRole::List)
        }) {
            assert!(
                !node.label.trim().is_empty(),
                "unnamed control on {screen:?}: {node:?}"
            );
        }
    }
}

#[test]
fn modal_requests_cannot_activate_a_board_square_or_change_the_game() {
    let mut driver = fixture("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
    let tree = snapshot(&mut driver);
    let id = tree
        .changed
        .iter()
        .find(|n| n.label == "e1, white king")
        .unwrap()
        .id;
    click(&mut driver, "a7");
    click(&mut driver, "a8");
    let before = driver.board().unwrap().game().clone();
    press(&mut driver, id);
    assert!(driver.promotion_pending());
    assert_eq!(driver.board().unwrap().game(), &before);
    assert!(
        snapshot(&mut driver)
            .changed
            .iter()
            .all(|n| n.label != "Chess board")
    );
}

#[test]
fn black_player_view_and_arrow_navigation_share_the_same_orientation() {
    let mut driver = fixture("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1");
    driver
        .set_computer(ComputerGame::new(Side::Onyx, 1, 342).unwrap())
        .unwrap();
    assert!(driver.camera_pose().unwrap().eye_millimeters[1] > 0);
    for _ in 0..6 {
        key(&mut driver, "arrowdown");
    }
    for _ in 0..3 {
        key(&mut driver, "arrowright");
    }
    key(&mut driver, "enter");
    assert_eq!(driver.selected(), Some(square("e7")));
    key(&mut driver, "arrowup");
    key(&mut driver, "arrowup");
    key(&mut driver, "enter");
    let mut expected =
        ChessState::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR b KQkq - 0 1").unwrap();
    expected.play(expected.parse_uci("e7e5").unwrap()).unwrap();
    assert_eq!(driver.board().unwrap().game(), &expected);
}

#[test]
fn resign_confirmation_and_draw_offer_are_named_and_announced() {
    let mut driver = fixture(crate::rules::INITIAL_FEN);
    driver
        .menu_action(super::menus::MenuAction::Resign)
        .unwrap();
    let update = snapshot(&mut driver);
    assert!(
        update
            .changed
            .iter()
            .any(|n| n.role == UiRole::Dialog && n.label == "Resign game?")
    );
    let confirm = update
        .changed
        .iter()
        .find(|n| n.label == "Confirm resign")
        .expect("named confirmation")
        .id;
    press(&mut driver, confirm);
    assert_eq!(
        snapshot(&mut driver).announcements[0].text,
        "Black wins: White resigned"
    );
    let mut driver = fixture(crate::rules::INITIAL_FEN);
    driver.transition(super::Screen::InGameMenu).unwrap();
    let update = snapshot(&mut driver);
    let offer = update
        .changed
        .iter()
        .find(|n| n.label == "Offer draw")
        .expect("draw offer control")
        .id;
    press(&mut driver, offer);
    let update = snapshot(&mut driver);
    assert_eq!(update.announcements[0].text, "White offers a draw");
    let accept = update
        .changed
        .iter()
        .find(|n| n.label == "Accept draw")
        .expect("draw acceptance")
        .id;
    press(&mut driver, accept);
    assert_eq!(
        snapshot(&mut driver).announcements[0].text,
        "Draw by agreement"
    );
}

use super::{BoardDriver, ComputerGame, RetainedBoard, viewer_options};
use crate::{CameraPreset, Side, Square, rules::ChessState};
use worldrunner::{
    scene::{
        gpu::{SceneView, Viewport},
        viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput},
    },
    ui::{UiAccessAction, UiAccessRequest, UiNodeId, UiRole, UiSemanticUpdate},
};

fn fixture(fen: &str) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::from_fen(fen).unwrap(), 342).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
}
fn snapshot(driver: &mut BoardDriver) -> UiSemanticUpdate {
    driver.sync_menu_ui().unwrap();
    driver
        .ui
        .as_mut()
        .unwrap()
        .semantic_update([800, 600])
        .unwrap()
}
fn key(driver: &mut BoardDriver, name: &str) {
    for pressed in [true, false] {
        driver
            .handle_input(
                &RetainedInput::Key {
                    key: name.into(),
                    logical: LogicalKey::Character(name.into()),
                    physical: KeyCode::Enter.into(),
                    pressed,
                    repeat: false,
                },
                None,
                None,
            )
            .unwrap();
    }
}
fn press(driver: &mut BoardDriver, id: UiNodeId) {
    driver
        .accessibility_input(UiAccessRequest {
            target: id,
            action: UiAccessAction::Press,
        })
        .unwrap();
}
fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').unwrap()
}
fn click(driver: &mut BoardDriver, name: &str) {
    let options = viewer_options(
        driver.board().unwrap().scene(),
        CameraPreset::Overhead,
        [800, 600],
    );
    let view = SceneView::new(
        options.pose,
        options.camera,
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    let s = square(name);
    let point = view
        .resolved_view()
        .world_to_screen(glam::Vec3::new(
            f32::from(s.file) - 3.5,
            f32::from(s.rank) - 3.5,
            0.05,
        ))
        .unwrap();
    driver
        .handle_input(
            &RetainedInput::PointerMoved([f64::from(point.x), f64::from(point.y)]),
            Some(&view),
            None,
        )
        .unwrap();
    for pressed in [true, false] {
        driver
            .handle_input(
                &RetainedInput::Button {
                    button: MouseButton::Left,
                    pressed,
                },
                Some(&view),
                None,
            )
            .unwrap();
    }
}

#[test]
fn starting_board_semantics_have_64_named_focusable_squares_in_player_order() {
    for human in [Side::Ivory, Side::Onyx] {
        let mut driver = fixture(crate::rules::INITIAL_FEN);
        driver
            .set_computer(ComputerGame::new(human, 1, 342).unwrap())
            .unwrap();
        let update = snapshot(&mut driver);
        let board = update
            .changed
            .iter()
            .find(|n| n.label == "Chess board")
            .expect("board group");
        assert_eq!(board.role, UiRole::Group);
        assert_eq!(board.children.len(), 64);
        for (index, id) in board.children.iter().enumerate() {
            let file = u8::try_from(index % 8).unwrap();
            let rank = u8::try_from(index / 8).unwrap();
            let location = if human == Side::Ivory {
                Square::new(file, 7 - rank)
            } else {
                Square::new(7 - file, rank)
            }
            .unwrap();
            let node = update.changed.iter().find(|n| &n.id == id).unwrap();
            let occupant = driver
                .board()
                .unwrap()
                .game()
                .piece_at(location)
                .map_or_else(
                    || "empty".to_owned(),
                    |(side, kind)| {
                        format!(
                            "{} {}",
                            super::status::side_name(side).to_lowercase(),
                            format!("{kind:?}").to_lowercase()
                        )
                    },
                );
            assert_eq!(
                node.label,
                format!(
                    "{}{}, {occupant}",
                    char::from(b'a' + location.file),
                    location.rank + 1
                )
            );
            assert_eq!(node.role, UiRole::Button);
            assert!(node.actions.contains(&UiAccessAction::Focus));
            assert!(node.actions.contains(&UiAccessAction::Press));
        }
    }
}

#[test]
fn keyboard_and_accessibility_activation_match_the_existing_pointer_path() {
    let mut keyboard = fixture(crate::rules::INITIAL_FEN);
    for _ in 0..6 {
        key(&mut keyboard, "arrowdown");
    }
    for _ in 0..4 {
        key(&mut keyboard, "arrowright");
    }
    key(&mut keyboard, "enter");
    assert_eq!(keyboard.selected(), Some(square("e2")));
    let update = snapshot(&mut keyboard);
    assert!(
        update
            .changed
            .iter()
            .any(|n| n.label == "e2, white pawn, selected")
    );
    assert!(
        update
            .changed
            .iter()
            .any(|n| n.label == "e4, empty, legal destination")
    );
    key(&mut keyboard, "arrowup");
    key(&mut keyboard, "arrowup");
    key(&mut keyboard, "space");
    let mut pointer = fixture(crate::rules::INITIAL_FEN);
    click(&mut pointer, "e2");
    click(&mut pointer, "e4");
    assert_eq!(
        keyboard.board().unwrap().game(),
        pointer.board().unwrap().game()
    );
    assert_eq!(
        keyboard.board().unwrap().objects(),
        pointer.board().unwrap().objects()
    );
    assert_eq!(keyboard.queue().borrow().len(), 1);
    let mut access = fixture(crate::rules::INITIAL_FEN);
    let update = snapshot(&mut access);
    let id = |name: &str| {
        update
            .changed
            .iter()
            .find(|n| n.label.starts_with(name))
            .unwrap()
            .id
    };
    press(&mut access, id("e2,"));
    press(&mut access, id("e4,"));
    assert_eq!(
        access.board().unwrap().game(),
        pointer.board().unwrap().game()
    );
    assert_eq!(
        access.board().unwrap().objects(),
        pointer.board().unwrap().objects()
    );
    assert_eq!(access.queue().borrow().len(), 1);
}

#[test]
fn live_status_announces_exact_chess_move_words() {
    for (fen, uci, expected) in [
        (
            crate::rules::INITIAL_FEN,
            "g1f3",
            "White knight from g1 to f3",
        ),
        (
            "4k3/8/8/8/8/8/4p3/4R1K1 w - - 0 1",
            "e1e2",
            "White rook from e1 to e2, captures black pawn, check",
        ),
        (
            "4k3/8/8/8/8/8/8/4K2R w K - 0 1",
            "e1g1",
            "White castles kingside, king from e1 to g1, rook from h1 to f1",
        ),
        (
            "4k3/P7/8/8/8/8/8/4K3 w - - 0 1",
            "a7a8q",
            "White pawn from a7 to a8, promotes to queen, check",
        ),
        (
            "7k/8/5KQ1/8/8/8/8/8 w - - 0 1",
            "g6g7",
            "White queen from g6 to g7, checkmate. White wins",
        ),
        (
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
            "e5d6",
            "White pawn from e5 to d6, captures black pawn en passant",
        ),
    ] {
        let mut driver = fixture(fen);
        snapshot(&mut driver);
        driver.play_uci(uci).unwrap();
        let update = snapshot(&mut driver);
        assert_eq!(
            update
                .announcements
                .iter()
                .map(|a| a.text.as_str())
                .collect::<Vec<_>>(),
            vec![expected]
        );
        let status = update.changed.iter().find(|n| n.label == expected).unwrap();
        assert_eq!(status.role, UiRole::Status);
        assert!(snapshot(&mut driver).announcements.is_empty());
    }
}

#[test]
fn promotion_dialog_and_save_field_have_accessible_names() {
    let mut driver = fixture("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
    click(&mut driver, "a7");
    click(&mut driver, "a8");
    let update = snapshot(&mut driver);
    assert!(
        update
            .changed
            .iter()
            .any(|n| n.role == UiRole::Dialog && n.label == "Promote pawn")
    );
    let names: Vec<_> = update
        .changed
        .iter()
        .filter(|n| n.actions.contains(&UiAccessAction::Press))
        .map(|n| n.label.as_str())
        .collect();
    assert_eq!(names, ["Queen", "Rook", "Bishop", "Knight"]);
    driver.clear_selection().unwrap();
    driver.transition(super::Screen::SaveLabel).unwrap();
    let update = snapshot(&mut driver);
    assert!(update.changed.iter().any(|n| n.role == UiRole::Text
        && n.label == "Save label"
        && n.actions.contains(&UiAccessAction::Focus)));
}
