mod lessons;

#[test]
fn selected_feedback_survives_shortlist_eviction_during_analysis() {
    let (mut driver, mut host) = setup();
    open(&mut driver);
    analyze(&mut driver, &mut host);
    click(&mut driver, [620., 286.]);
    let moment = driver
        .review
        .as_ref()
        .unwrap()
        .selected
        .as_ref()
        .unwrap()
        .moment
        .clone();
    let session = driver.review.as_mut().unwrap();
    session.report.moments.clear();
    super::feedback::update(&session.report.moments, &mut session.feedback);
    driver
        .handle_review_action(super::MenuAction::WhatChanged)
        .unwrap();
    assert!(driver.review_ui_state().message.contains("tactics.hanging"));
    driver
        .handle_review_action(super::MenuAction::TryAgain)
        .unwrap();
    driver.try_review_move(moment.played).unwrap();
    assert!(driver.review_ui_state().message.contains("Try again"));
    driver
        .handle_review_action(super::MenuAction::ShowReply)
        .unwrap();
}

#[test]
fn moment_feedback_is_cached_and_links_the_authored_lesson() {
    let (mut driver, mut host) = setup();
    open(&mut driver);
    analyze(&mut driver, &mut host);
    let session = driver.review.as_ref().unwrap();
    assert_eq!(session.feedback.len(), 1);
    let feedback = session.feedback.values().next().unwrap();
    assert!(feedback.learner.contains("queen"));
    assert!(feedback.learner.contains("tactics.hanging"));
    assert!(feedback.learner.contains("Count attackers and defenders"));
    assert!(feedback.learner.contains("open from Lessons"));
    let cached = session.feedback.clone();
    click(&mut driver, [620., 286.]);
    assert!(driver.review_ui_state().message.contains("tactics.hanging"));
    assert_eq!(driver.review.as_ref().unwrap().feedback, cached);
    key(&mut driver, "escape");
    open(&mut driver);
    assert_eq!(driver.review.as_ref().unwrap().feedback, cached);
}

use super::*;
use crate::{
    CameraPreset, Side,
    retained::{RetainedBoard, viewer_options},
    review::Label,
    search::Limits,
};
use worldrunner::scene::viewer::{KeyCode, LogicalKey, MouseButton, RetainedInput};

const QUEEN: &str = "4k3/8/8/8/r7/8/8/3QK3 w - - 0 1";
fn setup() -> (BoardDriver, RetainedHost) {
    let board = RetainedBoard::new(ChessState::from_fen(QUEEN).unwrap(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let host = RetainedHost::new(board.scene().clone(), options.clone()).unwrap();
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    for uci in ["d1d4", "a4d4"] {
        driver.play_uci(uci).unwrap();
    }
    driver.finish("Draw by agreement".into()).unwrap();
    (driver, host)
}
fn key(driver: &mut BoardDriver, key: &str) {
    for pressed in [true, false] {
        driver
            .handle_input(
                &RetainedInput::Key {
                    key: key.into(),
                    logical: LogicalKey::Character(key.into()),
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
        driver.handle_input(&input, None, None).unwrap();
    }
}
fn open(driver: &mut BoardDriver) {
    driver
        .open_review(ReviewConfig {
            limits: Limits {
                nodes: 5_000,
                depth: Some(2),
            },
            ..ReviewConfig::default()
        })
        .unwrap();
}
fn wait(driver: &mut BoardDriver) {
    let start = Instant::now();
    while driver.review_pending() {
        driver.drain_review();
        assert!(
            start.elapsed() < Duration::from_mins(1),
            "review job stalled"
        );
        std::thread::yield_now();
    }
}
fn analyze(driver: &mut BoardDriver, host: &mut RetainedHost) {
    while driver.review.as_ref().unwrap().progress.completed < 2 {
        driver.advance_review(host).unwrap();
        wait(driver);
    }
    driver.sync_menu_ui().unwrap();
}
#[test]
fn replay_controls_and_move_click_follow_record_not_analysis() {
    let (mut driver, _) = setup();
    let original = driver.board().unwrap().game().clone();
    open(&mut driver);
    click(&mut driver, [526., 140.]);
    assert_eq!(driver.review_cursor(), Some(0));
    assert_eq!(driver.board().unwrap().game().to_fen(), QUEEN);
    click(&mut driver, [674., 140.]);
    assert_eq!(driver.review_cursor(), Some(1));
    let first = driver.board().unwrap().game().clone();
    click(&mut driver, [748., 140.]);
    assert_eq!(driver.board().unwrap().game(), &original);
    click(&mut driver, [600., 140.]);
    assert_eq!(driver.board().unwrap().game(), &first);
    click(&mut driver, [526., 140.]);
    click(&mut driver, [620., 176.]);
    assert_eq!(driver.review_cursor(), Some(1));
    assert_eq!(driver.board().unwrap().game(), &first);
    key(&mut driver, "escape");
    assert_eq!(driver.board().unwrap().game(), &original);
}
#[test]
fn each_queue_tick_is_one_ply_and_leaving_pauses_then_resumes() {
    let (mut driver, mut host) = setup();
    open(&mut driver);
    driver.advance_review(&mut host).unwrap();
    assert!(driver.review_pending());
    // Input is handled while the worker owns the review core.
    click(&mut driver, [526., 140.]);
    assert_eq!(driver.review_cursor(), Some(0));
    wait(&mut driver);
    assert_eq!(driver.review.as_ref().unwrap().progress.completed, 1);
    let partial = driver.review_report().unwrap().clone();
    key(&mut driver, "escape");
    assert!(driver.review.as_ref().unwrap().stop.load(Ordering::Relaxed));
    driver.advance_review(&mut host).unwrap();
    assert!(!driver.review_pending());
    assert_eq!(driver.review_report(), Some(&partial));
    open(&mut driver);
    driver.advance_review(&mut host).unwrap();
    wait(&mut driver);
    assert_eq!(driver.review.as_ref().unwrap().progress.completed, 2);
    assert_eq!(&driver.review_report().unwrap().plies[..1], &partial.plies);
}
#[test]
fn grounded_blunder_moment_try_again_and_reply_preserve_the_record() {
    let (mut driver, mut host) = setup();
    let original = driver.board().unwrap().game().clone();
    let result = driver.status_text();
    let root = tempfile::tempdir().unwrap();
    driver.open_store(root.path().to_path_buf());
    driver.autosave();
    driver.save_game(1).unwrap();
    let saved = store_bytes(root.path());
    open(&mut driver);
    analyze(&mut driver, &mut host);
    let report = driver.review_report().unwrap();
    assert_eq!(report.moments.len(), 1);
    let moment = report.moments[0].clone();
    assert_eq!(moment.label, Label::Blunder);
    assert_eq!(moment.position_before.side_to_move(), Side::Ivory);
    assert!(driver.review_ui_state().moments[0].contains("Rxd4"));
    click(&mut driver, [620., 286.]);
    assert_eq!(driver.review_cursor(), Some(1));
    let reviewed = driver.board().unwrap().game().clone();
    click(&mut driver, [562., 430.]);
    assert!(driver.review_ui_state().message.contains("Search estimate"));
    click(&mut driver, [562., 467.]);
    assert!(driver.retry_active());
    assert_eq!(driver.board().unwrap().game(), &moment.position_before);
    pointer_move(&mut driver, moment.played);
    assert!(driver.retry_active());
    assert!(driver.review_status().unwrap().contains("Try again"));
    pointer_move(&mut driver, moment.better);
    assert!(!driver.retry_active());
    assert!(driver.review_status().unwrap().contains("Yes:"));
    click(&mut driver, [710., 467.]);
    assert_eq!(driver.board().unwrap().game(), &reviewed);
    click(&mut driver, [710., 430.]);
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    let now = Instant::now() + Duration::from_secs(2);
    driver.advance_reply(now).unwrap();
    let mut reply = moment.position_before.clone();
    reply.play(moment.played).unwrap();
    reply.play(moment.refutation.moves[0]).unwrap();
    assert_eq!(driver.board().unwrap().game(), &reply);
    driver.pending.borrow_mut().clear();
    driver.advance_reply(now + Duration::from_secs(2)).unwrap();
    assert_eq!(driver.board().unwrap().game(), &reviewed);
    key(&mut driver, "escape");
    assert_eq!(driver.board().unwrap().game(), &original);
    assert_eq!(driver.status_text(), result);
    assert_eq!(store_bytes(root.path()), saved);
}

fn pointer_move(driver: &mut BoardDriver, chess_move: LegalMove) {
    use worldrunner::scene::gpu::{SceneView, Viewport};
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
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    for square in [chess_move.from(), chess_move.to()] {
        let point = view
            .resolved_view()
            .world_to_screen(glam::Vec3::new(
                f32::from(square.file) - 3.5,
                f32::from(square.rank) - 3.5,
                0.05,
            ))
            .unwrap();
        for input in [
            RetainedInput::PointerMoved([f64::from(point.x), f64::from(point.y)]),
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
}
fn store_bytes(root: &std::path::Path) -> Vec<(std::path::PathBuf, Vec<u8>)> {
    let mut files: Vec<_> = std::fs::read_dir(root)
        .unwrap()
        .map(|entry| {
            let path = entry.unwrap().path();
            let bytes = std::fs::read(&path).unwrap();
            (path, bytes)
        })
        .collect();
    files.sort();
    files
}
#[test]
fn leaving_with_a_pending_ply_cancels_before_the_next_ply() {
    let (mut driver, mut host) = setup();
    open(&mut driver);
    driver.advance_review(&mut host).unwrap();
    assert!(driver.review_pending());
    key(&mut driver, "escape");
    wait(&mut driver);
    let progress = driver.review.as_ref().unwrap().progress;
    assert!(progress.completed <= 1);
    driver.advance_review(&mut host).unwrap();
    assert_eq!(driver.review.as_ref().unwrap().progress, progress);
    open(&mut driver);
    analyze(&mut driver, &mut host);
    assert_eq!(driver.review_report().unwrap().plies.len(), 2);
}
#[test]
fn unclear_analysis_keeps_every_move_accessible_in_plain_words() {
    let (mut driver, mut host) = setup();
    driver
        .open_review(ReviewConfig {
            limits: Limits {
                nodes: 1,
                depth: None,
            },
            ..ReviewConfig::default()
        })
        .unwrap();
    analyze(&mut driver, &mut host);
    let state = driver.review_ui_state();
    assert_eq!(state.moves.len(), 2);
    assert!(
        state
            .moves
            .iter()
            .all(|s| s.contains("unclear: analysis cannot judge this move"))
    );
    assert!(state.moments.is_empty());
}
