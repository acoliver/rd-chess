use super::*;
use crate::retained::{ComputerGame, menus::MenuAction};

fn selected() -> (BoardDriver, RetainedHost) {
    let (mut driver, mut host) = setup();
    open(&mut driver);
    analyze(&mut driver, &mut host);
    click(&mut driver, [620., 286.]);
    (driver, host)
}
fn unlock_related(driver: &mut BoardDriver) {
    let training = driver.training.as_mut().unwrap();
    let lesson = training.course.lesson("tactics.hanging").unwrap();
    training
        .progress
        .completed_lessons
        .extend(lesson.requires.clone());
}

#[test]
fn locked_related_lesson_names_its_unfinished_prerequisite_without_leaving_review() {
    let (mut driver, mut host) = selected();
    let board = driver.board().unwrap().game().clone();
    let report = driver.review_report().unwrap().clone();
    let camera = driver.camera_pose();
    let progress = driver.progress().unwrap().clone();
    click(&mut driver, [630., 541.]);
    assert_eq!(driver.screen(), Screen::Review);
    let message = driver.review_ui_state().message;
    assert!(message.contains("tactics.hanging"), "{message}");
    assert!(message.contains("locked"), "{message}");
    let training = driver.training.as_ref().unwrap();
    for id in &training.course.lesson("tactics.hanging").unwrap().requires {
        let prerequisite = training.course.lesson(id).unwrap();
        assert!(message.contains(&prerequisite.id), "{message}");
        assert!(message.contains(&prerequisite.title), "{message}");
    }
    assert_eq!(driver.board().unwrap().game(), &board);
    assert_eq!(driver.review_report(), Some(&report));
    assert_eq!(driver.camera_pose(), camera);
    assert_eq!(driver.progress(), Some(&progress));
    driver.advance_review(&mut host).unwrap();
}

#[test]
fn related_lesson_pointer_and_keyboard_exit_restore_review_and_saved_game_bytes() {
    for escape in [false, true] {
        let (mut driver, mut host) = selected();
        let root = tempfile::tempdir().unwrap();
        driver.open_store(root.path().to_path_buf());
        unlock_related(&mut driver);
        driver.persist_training_progress();
        let before = driver.board().unwrap().game().clone();
        let identities = driver.board().unwrap().identities().clone();
        let camera = driver.camera_pose();
        let cursor = driver.review_cursor();
        let report = driver.review_report().unwrap().clone();
        let result = driver.menus.result.clone();
        let saved = store_bytes(root.path());
        click(&mut driver, [630., 541.]);
        assert_eq!(driver.screen(), Screen::Lesson);
        assert!(
            driver
                .panel_text()
                .starts_with("Count attackers and defenders")
        );
        assert!(driver.review.as_ref().unwrap().stop.load(Ordering::Relaxed));
        driver.advance_review(&mut host).unwrap();
        assert!(!driver.review_pending());
        assert_eq!(driver.review_report(), Some(&report));
        if escape {
            key(&mut driver, "escape");
        } else {
            click(&mut driver, [720., 565.]);
        }
        assert_eq!(driver.screen(), Screen::Review);
        assert_eq!(driver.board().unwrap().game(), &before);
        assert_eq!(driver.board().unwrap().identities(), &identities);
        assert_eq!(driver.review_report(), Some(&report));
        assert_eq!(driver.review_cursor(), cursor);
        assert_eq!(driver.camera_pose(), camera);
        assert_eq!(driver.menus.result, result);
        assert!(!driver.review.as_ref().unwrap().stop.load(Ordering::Relaxed));
        assert!(driver.panel_text().is_empty());
        assert!(driver.training_highlights().is_empty());
        assert_eq!(store_bytes(root.path()), saved);
        driver
            .handle_review_action(MenuAction::WhatChanged)
            .unwrap();
        assert!(driver.review_ui_state().message.contains("tactics.hanging"));
        driver.handle_review_action(MenuAction::TryAgain).unwrap();
        assert!(driver.retry_active());
        let better = driver.review_report().unwrap().moments[0].better;
        pointer_move(&mut driver, better);
        assert!(driver.review_ui_state().message.contains("Yes:"));
        key(&mut driver, "escape");
        assert_eq!(driver.screen(), Screen::GameOver);
        assert_eq!(
            driver.board().unwrap().game(),
            &driver.review.as_ref().unwrap().recorded
        );
    }
}

#[test]
fn completing_linked_lesson_returns_to_review_and_persists_only_training_progress() {
    let (mut driver, mut host) = setup();
    let root = tempfile::tempdir().unwrap();
    driver.open_store(root.path().to_path_buf());
    driver.autosave();
    driver.save_game(1).unwrap();
    open(&mut driver);
    analyze(&mut driver, &mut host);
    driver.handle_review_action(MenuAction::Moment(0)).unwrap();
    driver
        .review
        .as_mut()
        .unwrap()
        .selected
        .as_mut()
        .unwrap()
        .explanation
        .motif = Some(crate::teaching::explain::Motif::Promotion);
    let before = driver.board().unwrap().game().clone();
    let identities = driver.board().unwrap().identities().clone();
    let camera = driver.camera_pose();
    let report = driver.review_report().unwrap().clone();
    let saved = store_bytes(root.path());
    click(&mut driver, [630., 541.]);
    assert_eq!(driver.screen(), Screen::Lesson);
    assert!(driver.panel_text().contains("Pawns, knights and kings"));
    for uci in ["e2e4", "g1f3", "e1d1"] {
        driver.training_action(MenuAction::StudyNext).unwrap();
        let mv = driver.board().unwrap().game().parse_uci(uci).unwrap();
        pointer_move(&mut driver, mv);
        assert!(driver.panel_text().contains("Completed"));
        driver.training_action(MenuAction::StudyNext).unwrap();
    }
    assert_eq!(driver.screen(), Screen::Review);
    assert_eq!(driver.board().unwrap().game(), &before);
    assert_eq!(driver.board().unwrap().identities(), &identities);
    assert_eq!(driver.camera_pose(), camera);
    assert_eq!(driver.review_report(), Some(&report));
    assert!(
        driver
            .progress()
            .unwrap()
            .completed_lessons
            .contains("rules.first-moves")
    );
    let mut reopened =
        BoardDriver::with_board(RetainedBoard::new(ChessState::initial(), 42).unwrap());
    reopened.open_store(root.path().to_path_buf());
    reopened
        .configure(&viewer_options(
            reopened.board().unwrap().scene(),
            CameraPreset::Overhead,
            [800, 600],
        ))
        .unwrap();
    assert_eq!(reopened.progress(), driver.progress());
    for (path, bytes) in saved {
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
    }
}

#[test]
fn linked_lesson_preserves_computer_configuration_and_pending_review_work() {
    let (mut driver, mut host) = setup();
    driver
        .set_computer(ComputerGame::new(Side::Ivory, 3, 42).unwrap())
        .unwrap();
    open(&mut driver);
    driver.advance_review(&mut host).unwrap();
    wait(&mut driver);
    driver.handle_review_action(MenuAction::Moment(0)).unwrap();
    unlock_related(&mut driver);
    driver.advance_review(&mut host).unwrap();
    assert!(driver.review_pending());
    let config = driver.computer_game().unwrap();
    click(&mut driver, [630., 541.]);
    assert_eq!(driver.screen(), Screen::Lesson);
    wait(&mut driver);
    let report = driver.review_report().unwrap().clone();
    driver.advance_review(&mut host).unwrap();
    assert!(!driver.review_pending());
    key(&mut driver, "escape");
    assert_eq!(driver.screen(), Screen::Review);
    let restored = driver.computer_game().unwrap();
    assert_eq!(
        (restored.human(), restored.level(), restored.seed()),
        (config.human(), config.level(), config.seed())
    );
    assert_eq!(driver.review_report(), Some(&report));
    assert_eq!(driver.review_cursor(), Some(1));
}
