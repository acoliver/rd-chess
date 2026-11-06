mod authored;
use super::*;
use crate::{
    CameraPreset,
    retained::{RetainedBoard, Screen, menus::MenuAction, viewer_options},
    rules::ChessState,
};
use glam::Vec3;
use worldrunner::ui::plan::ui::annotations::{
    AnnotationDepth, AnnotationPoint, AnnotationPrimitive,
};

fn fixture() -> BoardDriver {
    let board = RetainedBoard::new(ChessState::initial(), 42).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Overhead, [800, 600]);
    let mut driver = BoardDriver::with_board(board);
    driver.configure(&options).unwrap();
    driver
}
fn action(driver: &mut BoardDriver, action: MenuAction) {
    driver.training_action(action).unwrap();
}
fn point(file: u8, rank: u8) -> AnnotationPoint {
    AnnotationPoint::World(Vec3::new(
        f32::from(file) - 3.5,
        f32::from(rank) - 3.5,
        0.065,
    ))
}
fn arrow(annotation: &Annotation, from: AnnotationPoint, to: AnnotationPoint, expected: [f32; 4]) {
    let AnnotationPrimitive::Arrow {
        from: actual_from,
        to: actual_to,
        colour,
        ..
    } = annotation.primitive
    else {
        panic!("expected arrow: {annotation:?}");
    };
    assert_eq!(actual_from, from);
    assert_eq!(actual_to, to);
    assert_eq!(colour.map(f32::to_bits), expected.map(f32::to_bits));
    assert_eq!(annotation.depth, AnnotationDepth::DepthTested);
}
#[test]
fn lesson_annotations_map_board_colours_and_clear_on_step_and_exit() {
    let mut driver = fixture();
    action(&mut driver, MenuAction::Lesson(0));
    assert!(driver.board_annotations().is_empty());
    action(&mut driver, MenuAction::StudyNext);
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), 3);
    for (annotation, rank) in annotations.iter().zip(1..=3) {
        let AnnotationPrimitive::Circle {
            centre,
            thickness,
            colour,
            ..
        } = annotation.primitive
        else {
            panic!("expected ring");
        };
        assert_eq!(centre, point(4, rank));
        assert!(thickness.is_some());
        assert_eq!(
            colour.map(f32::to_bits),
            [0., 0.7, 0.8, 1.].map(f32::to_bits)
        );
        assert_eq!(annotation.depth, AnnotationDepth::DepthTested);
    }
    let mv = driver.board().unwrap().game().parse_uci("e2e4").unwrap();
    driver.submit_training_move(mv).unwrap();
    action(&mut driver, MenuAction::StudyNext);
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), 1);
    arrow(
        &annotations[0],
        point(6, 0),
        point(5, 2),
        [0., 0.7, 0.8, 1.],
    );
    assert!(!driver.panel_text().to_lowercase().contains("arrows:"));
    action(&mut driver, MenuAction::StudyExit);
    assert!(driver.board_annotations().is_empty());
}
#[test]
fn hints_reveal_piece_then_move_and_undo_clears_them() {
    let mut driver = fixture();
    action(&mut driver, MenuAction::Lesson(0));
    action(&mut driver, MenuAction::StudyNext);
    let authored = driver.board_annotations().len();
    action(&mut driver, MenuAction::StudyHint);
    assert_eq!(driver.board_annotations().len(), authored);
    action(&mut driver, MenuAction::StudyHint);
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), authored + 1);
    assert!(
        matches!(annotations[authored].primitive, AnnotationPrimitive::Circle { centre, .. } if centre == point(4, 1))
    );
    action(&mut driver, MenuAction::StudyHint);
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), authored + 2);
    arrow(
        &annotations[authored + 1],
        point(4, 1),
        point(4, 2),
        [0., 0.168_269_4, 0.445_201_2, 1.],
    );
    let mv = driver.board().unwrap().game().parse_uci("e2e3").unwrap();
    driver.submit_training_move(mv).unwrap();
    driver.undo().unwrap();
    assert_eq!(driver.screen(), Screen::Play);
    assert!(driver.board_annotations().is_empty());
    assert!(driver.training_highlights().is_empty());
}
#[test]
fn puzzle_hints_follow_the_current_solver_checkpoint() {
    let mut driver = fixture();
    let puzzle = driver.training.as_ref().unwrap().puzzles.puzzles()[0].clone();
    driver.begin_puzzle(puzzle).unwrap();
    assert!(driver.board_annotations().is_empty());
    driver.pending.borrow_mut().clear();
    driver.presenting = false;
    driver.advance_training(std::time::Duration::ZERO).unwrap();
    driver.pending.borrow_mut().clear();
    driver
        .advance_training(std::time::Duration::from_secs(1))
        .unwrap();
    for level in 1..=3 {
        action(&mut driver, MenuAction::StudyHint);
        assert_eq!(driver.board_annotations().len(), level - 1);
    }
    let annotations = driver.board_annotations();
    arrow(
        &annotations[1],
        point(4, 5),
        point(4, 6),
        [0., 0.168_269_4, 0.445_201_2, 1.],
    );
    let mv = driver.board().unwrap().game().parse_uci("e6e7").unwrap();
    driver.submit_training_move(mv).unwrap();
    assert!(driver.board_annotations().is_empty());
    for _ in 0..3 {
        action(&mut driver, MenuAction::StudyHint);
    }
    let annotations = driver.board_annotations();
    arrow(
        &annotations[1],
        point(1, 2),
        point(2, 0),
        [0., 0.168_269_4, 0.445_201_2, 1.],
    );
    action(&mut driver, MenuAction::StudyExit);
    assert!(driver.board_annotations().is_empty());
    assert!(driver.training_highlights().is_empty());
}
