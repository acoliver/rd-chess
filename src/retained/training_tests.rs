#[test]
fn loading_a_saved_game_clears_the_previous_reply_explanation() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture();
    driver.open_store(root.path().to_path_buf());
    driver.menu_action(MenuAction::Friend).unwrap();
    driver.save_game(1).unwrap();
    let before = ChessState::initial();
    let mv = before.parse_uci("e2e4").unwrap();
    driver.training.as_mut().unwrap().reply = Some((before, mv));
    driver.explain_computer_reply().unwrap();
    let study = driver.study_ui_state().unwrap();
    assert!(study.why);
    assert!(study.emphasis.iter().any(|san| san == "e4"));
    assert!(driver.panel_text().starts_with("Why e4?"));
    driver.load_game(1).unwrap();
    assert!(!driver.study_ui_state().unwrap().why);
    assert!(driver.panel_text().is_empty());
}

#[test]
fn training_checkpoints_preserve_saved_games_and_return_to_continue() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture();
    driver.open_store(root.path().to_path_buf());
    driver.menu_action(MenuAction::Friend).unwrap();
    driver.play_uci("e2e4").unwrap();
    let game = driver.board().unwrap().game().clone();
    driver.save_game(1).unwrap();
    driver.show_title().unwrap();
    start_puzzle(&mut driver);
    move_click(&mut driver, "e6e7");
    driver.show_title().unwrap();
    driver.continue_game().unwrap();
    assert_eq!(driver.board().unwrap().game(), &game);
    assert_eq!(driver.saved_payload(1).unwrap().replay().unwrap(), game);
    assert!(driver.markers.base >= 68);
    assert_eq!(driver.squares.base, driver.markers.base + 64);
    assert!(driver.squares.visible.is_empty());
    driver.show_title().unwrap();
    lessons(&mut driver);
    key(&mut driver, "enter");
    next(&mut driver);
    move_click(&mut driver, "e2e4");
    let mut reopened = fixture();
    reopened.open_store(root.path().to_path_buf());
    reopened.continue_game().unwrap();
    assert_eq!(reopened.board().unwrap().game(), &game);
}

#[test]
fn status_is_hidden_on_browsers_and_shown_on_board_screens() {
    let mut driver = fixture();
    for screen in [
        Screen::Title,
        Screen::ComputerSetup,
        Screen::Lessons,
        Screen::Puzzles,
    ] {
        driver.transition(screen).unwrap();
        assert!(driver.status_text().is_empty(), "status on {screen:?}");
    }
    for screen in [
        Screen::Play,
        Screen::Lesson,
        Screen::Puzzle,
        Screen::InGameMenu,
        Screen::GameOver,
    ] {
        driver.transition(screen).unwrap();
        assert_eq!(
            driver.status_text(),
            "White to move",
            "status on {screen:?}"
        );
    }
}

#[test]
fn locked_lessons_keep_their_marker_and_cannot_be_opened() {
    let mut driver = fixture();
    let state = driver.study_ui_state().unwrap();
    assert!(state.lessons[0].1);
    assert!(state.lessons[0].0.starts_with("[open]"));
    assert!(!state.lessons[1].1);
    assert!(state.lessons[1].0.starts_with("[locked]"));
    driver.transition(Screen::Lessons).unwrap();
    driver.training_action(MenuAction::Lesson(1)).unwrap();
    assert_eq!(driver.screen(), Screen::Lessons);
}

#[test]
fn puzzle_setup_delay_blocks_solver_and_escape_restores_game_camera() {
    let mut driver = fixture();
    let camera = driver.camera_pose();
    for name in [
        "home",
        "arrowdown",
        "arrowdown",
        "arrowdown",
        "enter",
        "enter",
    ] {
        key(&mut driver, name);
    }
    let raw = driver.board().unwrap().game().to_fen();
    move_click(&mut driver, "e6e7");
    assert_eq!(driver.board().unwrap().game().to_fen(), raw);
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    driver.advance_training(Duration::ZERO).unwrap();
    driver.advance_training(Duration::from_millis(349)).unwrap();
    assert_eq!(driver.board().unwrap().game().to_fen(), raw);
    driver.advance_training(Duration::from_millis(350)).unwrap();
    assert_ne!(driver.board().unwrap().game().to_fen(), raw);
    key(&mut driver, "escape");
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), crate::retained::Screen::Title);
    assert_eq!(driver.camera_pose(), camera);
    assert_eq!(driver.board().unwrap().game(), &ChessState::initial());
}

use super::*;
use crate::{
    CameraPreset,
    retained::{RetainedBoard, Screen, viewer_options},
    rules::ChessState,
};
use worldrunner::scene::{
    gpu::{SceneView, Viewport},
    viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput},
};

fn fixture() -> BoardDriver {
    fixture_store(None)
}
fn fixture_store(root: Option<&std::path::Path>) -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    if let Some(root) = root {
        driver.open_store(root.to_path_buf());
    }
    driver.configure(&options).unwrap();
    driver.show_title().unwrap();
    driver
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
fn click(driver: &mut BoardDriver, point: [f64; 2]) {
    let options = viewer_options(
        driver.board().unwrap().scene(),
        CameraPreset::Overhead,
        [800, 600],
    );
    let view = SceneView::new(
        driver.camera_pose().unwrap(),
        options.camera,
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    for input in [
        RetainedInput::PointerMoved(point),
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: true,
        },
        RetainedInput::Button {
            button: MouseButton::Left,
            pressed: false,
        },
    ] {
        driver.handle_input(&input, Some(&view), None).unwrap();
    }
}
fn move_click(driver: &mut BoardDriver, uci: &str) {
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    let options = viewer_options(
        driver.board().unwrap().scene(),
        CameraPreset::Overhead,
        [800, 600],
    );
    let view = SceneView::new(
        driver.camera_pose().unwrap(),
        options.camera,
        Viewport {
            width: 800,
            height: 600,
        },
    )
    .unwrap();
    for name in [&uci[..2], &uci[2..4]] {
        let bytes = name.as_bytes();
        let p = view
            .resolved_view()
            .world_to_screen(glam::Vec3::new(
                f32::from(bytes[0] - b'a') - 3.5,
                f32::from(bytes[1] - b'1') - 3.5,
                0.05,
            ))
            .unwrap();
        click(driver, [f64::from(p.x), f64::from(p.y)]);
    }
}
fn lessons(driver: &mut BoardDriver) {
    for name in ["home", "arrowdown", "arrowdown", "enter"] {
        key(driver, name);
    }
}
fn next(driver: &mut BoardDriver) {
    click(driver, [240., 565.]);
}

#[test]
fn locked_lesson_cannot_open_by_pointer_and_list_has_twelve_rows() {
    let mut driver = fixture();
    lessons(&mut driver);
    assert_eq!(driver.training.as_ref().unwrap().course.lessons().len(), 12);
    click(&mut driver, [400., 235.]);
    assert_eq!(driver.screen(), super::super::Screen::Lessons);
    key(&mut driver, "home");
    key(&mut driver, "enter");
    assert_eq!(driver.screen(), super::super::Screen::Lesson);
}
#[test]
fn lesson_wrong_feedback_hints_and_completion_use_board_clicks() {
    let mut driver = fixture();
    lessons(&mut driver);
    key(&mut driver, "enter");
    next(&mut driver);
    let before = driver.board().unwrap().game().clone();
    move_click(&mut driver, "d2d4");
    assert_eq!(driver.board().unwrap().game(), &before);
    assert!(driver.panel_text().contains("d-pawn"));
    click(&mut driver, [400., 565.]);
    assert!(driver.panel_text().contains("e2"));
    let first = driver.panel_text().to_string();
    click(&mut driver, [400., 565.]);
    assert_ne!(driver.panel_text(), first);
    let final_hint = driver.panel_text().to_owned();
    click(&mut driver, [400., 565.]);
    assert_eq!(driver.panel_text(), final_hint);
    move_click(&mut driver, "e2e4");
    next(&mut driver);
    next(&mut driver);
    move_click(&mut driver, "g1f3");
    next(&mut driver);
    next(&mut driver);
    move_click(&mut driver, "e1d1");
    next(&mut driver);
    assert!(
        driver
            .progress()
            .unwrap()
            .completed_lessons
            .contains("rules.first-moves")
    );
    assert_eq!(
        driver.progress().unwrap().lessons["rules.first-moves"].hints_used,
        2
    );
    assert_eq!(driver.screen(), super::super::Screen::Lessons);
    assert_eq!(
        driver.progress().unwrap().lessons["rules.first-moves"]
            .completed_steps
            .len(),
        6
    );
    key(&mut driver, "arrowdown");
    key(&mut driver, "enter");
    assert!(driver.panel_text().contains("Rooks, bishops and queens"));
}
#[test]
fn lesson_scripted_reply_and_back_restore_checkpoint() {
    let mut driver = fixture();
    let training = driver.training.as_mut().unwrap();
    for lesson in training.course.lessons() {
        training
            .progress
            .completed_lessons
            .insert(lesson.id.clone());
    }
    lessons(&mut driver);
    for _ in 0..4 {
        key(&mut driver, "arrowdown");
    }
    key(&mut driver, "enter");
    next(&mut driver);
    move_click(&mut driver, "a5a7");
    assert_eq!(
        driver.board().unwrap().game().side_to_move(),
        crate::Side::Ivory
    );
    assert_eq!(driver.board().unwrap().game().repetition_history().len(), 3);
    assert!(driver.panel_text().contains("Correct"));
    click(&mut driver, [80., 565.]);
    next(&mut driver);
    assert_eq!(driver.board().unwrap().game().repetition_history().len(), 1);
}
fn start_puzzle(driver: &mut BoardDriver) {
    for name in [
        "home",
        "arrowdown",
        "arrowdown",
        "arrowdown",
        "enter",
        "enter",
    ] {
        key(driver, name);
    }
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    driver.advance_training(Duration::ZERO).unwrap();
    driver.advance_training(Duration::from_millis(400)).unwrap();
}
#[test]
fn puzzle_setup_wrong_retry_hint_and_solver_line() {
    let mut driver = fixture();
    start_puzzle(&mut driver);
    let before = driver.board().unwrap().game().clone();
    let puzzle = PuzzlePack::bundled().unwrap().puzzles()[0].clone();
    assert_eq!(before, puzzle.after_setup().unwrap());
    move_click(&mut driver, "h6h5");
    assert_eq!(driver.board().unwrap().game(), &before);
    assert!(driver.panel_text().contains("Try again"));
    let explanation = crate::teaching::explain_move(
        &before,
        before.parse_uci("h6h5").unwrap(),
        crate::teaching::Budget::default(),
    )
    .unwrap();
    assert!(
        !explanation.facts.is_empty(),
        "wrong move fixture needs a verified fact"
    );
    for sentence in explanation.sentences {
        assert!(driver.panel_text().contains(&sentence));
    }
    click(&mut driver, [400., 565.]);
    assert!(
        driver
            .training_highlights()
            .contains(&crate::Square::new(4, 5).unwrap())
    );
    click(&mut driver, [560., 565.]);
    assert_eq!(driver.board().unwrap().game(), &before);
    for uci in ["e6e7", "b3c1", "h6c1"] {
        move_click(&mut driver, uci);
    }
    assert!(driver.panel_text().contains("Completed"));
    assert_eq!(driver.progress().unwrap().bands["1800-plus"].solved, 1);
}
#[test]
fn puzzle_alternative_mate_is_accepted_in_window() {
    let mut driver = fixture();
    let pack = PuzzlePack::bundled().unwrap();
    let puzzle = pack
        .puzzles()
        .iter()
        .find(|p| {
            if !p.themes.contains("mateIn1") {
                return false;
            }
            let state = p.after_setup().unwrap();
            state.legal_moves().iter().any(|&mv| {
                let mut after = state.clone();
                after.play(mv).unwrap();
                matches!(after.status(), crate::rules::GameStatus::Checkmate { .. })
                    && mv.to_uci() != p.moves.split_whitespace().nth(1).unwrap()
            })
        })
        .unwrap()
        .clone();
    driver.begin_puzzle(puzzle.clone()).unwrap();
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    driver.advance_training(Duration::ZERO).unwrap();
    driver.advance_training(Duration::from_secs(1)).unwrap();
    let state = puzzle.after_setup().unwrap();
    let mv = state
        .legal_moves()
        .into_iter()
        .find(|&mv| {
            let mut after = state.clone();
            after.play(mv).unwrap();
            matches!(after.status(), crate::rules::GameStatus::Checkmate { .. })
                && mv.to_uci() != puzzle.moves.split_whitespace().nth(1).unwrap()
        })
        .unwrap();
    move_click(&mut driver, &mv.to_uci());
    assert!(driver.panel_text().contains("Completed"));
}

#[test]
fn lesson_and_puzzle_progress_survives_application_store_round_trip() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture_store(Some(root.path()));
    lessons(&mut driver);
    key(&mut driver, "enter");
    next(&mut driver);
    click(&mut driver, [400., 565.]);
    move_click(&mut driver, "e2e4");
    next(&mut driver);
    next(&mut driver);
    move_click(&mut driver, "g1f3");
    next(&mut driver);
    next(&mut driver);
    move_click(&mut driver, "e1d1");
    next(&mut driver);
    assert!(
        driver
            .progress()
            .unwrap()
            .completed_lessons
            .contains("rules.first-moves")
    );
    let mut reopened = fixture_store(Some(root.path()));
    assert_eq!(reopened.progress(), driver.progress());
    assert!(reopened.study_ui_state().unwrap().lessons[1].1);
    start_puzzle(&mut reopened);
    for uci in ["e6e7", "b3c1", "h6c1"] {
        move_click(&mut reopened, uci);
    }
    assert_eq!(reopened.progress().unwrap().bands["1800-plus"].solved, 1);
    let next = fixture_store(Some(root.path()));
    assert_eq!(next.progress(), reopened.progress());
    assert!(next.progress_error().is_none());
}

#[test]
fn changed_progress_is_written_before_leaving_the_training_screen() {
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture_store(Some(root.path()));
    lessons(&mut driver);
    key(&mut driver, "enter");
    next(&mut driver);
    click(&mut driver, [400., 565.]);
    assert_eq!(
        driver.progress().unwrap().lessons["rules.first-moves"].hints_used,
        1
    );
    let next = fixture_store(Some(root.path()));
    assert_eq!(next.progress(), driver.progress());
    assert_eq!(driver.screen(), Screen::Lesson);
}

#[test]
fn corrupt_progress_is_typed_preserved_and_does_not_break_saved_games() {
    use crate::retained::persistence::ProgressError;
    let root = tempfile::tempdir().unwrap();
    let mut driver = fixture_store(Some(root.path()));
    driver.menu_action(MenuAction::Friend).unwrap();
    driver.play_uci("e2e4").unwrap();
    driver.save_game(1).unwrap();
    let game = driver.board().unwrap().game().clone();
    let path = root.path().join("slot-training-progress.rds");
    std::fs::write(&path, b"damaged progress").unwrap();
    let mut next = fixture_store(Some(root.path()));
    assert!(matches!(
        next.progress_error(),
        Some(ProgressError::Store(_))
    ));
    assert!(next.can_continue());
    assert_eq!(next.slot_labels()[0], "Two players, move 1");
    next.continue_game().unwrap();
    assert_eq!(next.board().unwrap().game(), &game);
    next.load_game(1).unwrap();
    assert_eq!(next.board().unwrap().game(), &game);
    next.show_title().unwrap();
    lessons(&mut next);
    key(&mut next, "enter");
    click(&mut next, [240., 565.]);
    assert!(matches!(
        next.progress_error(),
        Some(ProgressError::Store(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"damaged progress");
    next.show_title().unwrap();
    next.continue_game().unwrap();
    next.play_uci("e7e5").unwrap();
    next.save_game(2).unwrap();
    assert_eq!(next.saved_payload(2).unwrap().moves, ["e2e4", "e7e5"]);
}
