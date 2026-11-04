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
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
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
