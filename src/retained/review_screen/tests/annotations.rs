use super::*;
use worldrunner::ui::plan::ui::annotations::{
    AnnotationDepth, AnnotationPoint, AnnotationPrimitive,
};

fn assert_move(
    annotation: &worldrunner::ui::plan::ui::annotations::Annotation,
    mv: LegalMove,
    expected: [f32; 4],
) {
    let AnnotationPrimitive::Arrow {
        from, to, colour, ..
    } = annotation.primitive
    else {
        panic!("expected move arrow");
    };
    let point = |square: crate::Square| {
        AnnotationPoint::World(glam::Vec3::new(
            f32::from(square.file) - 3.5,
            f32::from(square.rank) - 3.5,
            0.065,
        ))
    };
    assert_eq!(
        (from, to, colour),
        (point(mv.from()), point(mv.to()), expected)
    );
    assert_eq!(annotation.depth, AnnotationDepth::DepthTested);
}
#[test]
fn review_publishes_suggestion_and_threat_with_labels_and_clears_on_seek_and_exit() {
    let (mut driver, mut host) = setup();
    open(&mut driver);
    analyze(&mut driver, &mut host);
    driver.handle_review_action(MenuAction::Moment(0)).unwrap();
    let moment = driver.review_report().unwrap().moments[0].clone();
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), 4);
    assert_move(
        &annotations[0],
        moment.better,
        [0., 0.168_269_4, 0.445_201_2, 1.],
    );
    assert_move(
        &annotations[2],
        moment.refutation.moves[0],
        [0.665_387_3, 0.111_932_4, 0., 1.],
    );
    for (index, text) in [(1, "Better"), (3, "Threat")] {
        assert!(
            matches!(&annotations[index].primitive, AnnotationPrimitive::Label { run, .. } if run.text == text)
        );
    }
    driver
        .handle_review_action(MenuAction::WhatChanged)
        .unwrap();
    assert_eq!(driver.board_annotations(), annotations);
    driver.seek_review(0).unwrap();
    assert!(driver.board_annotations().is_empty());
    driver.handle_review_action(MenuAction::Moment(0)).unwrap();
    key(&mut driver, "escape");
    assert!(driver.board_annotations().is_empty());
}
#[test]
fn why_uses_grounded_alternative_and_reply_arrows_then_clears_on_undo() {
    let (mut driver, _) = setup();
    driver.undo().unwrap();
    driver.undo().unwrap();
    let state = driver.board().unwrap().game().clone();
    let mv = state.parse_uci("d1d4").unwrap();
    driver.play_uci("d1d4").unwrap();
    let explanation =
        crate::teaching::explain_reply(&state, mv, crate::teaching::Budget::default()).unwrap();
    driver.training.as_mut().unwrap().reply = Some((state, mv));
    driver.training_action(MenuAction::Why).unwrap();
    let annotations = driver.board_annotations();
    assert_eq!(annotations.len(), 4);
    assert_move(
        &annotations[0],
        explanation.alternative_line.unwrap()[0],
        [0., 0.168_269_4, 0.445_201_2, 1.],
    );
    assert_move(
        &annotations[2],
        explanation.line[1],
        [0.665_387_3, 0.111_932_4, 0., 1.],
    );
    driver.undo().unwrap();
    assert!(driver.board_annotations().is_empty());
}

#[test]
fn why_does_not_label_a_quiet_search_reply_as_a_threat() {
    let (mut driver, _) = setup();
    driver.undo().unwrap();
    driver.undo().unwrap();
    let state = ChessState::initial();
    let mv = state.parse_uci("e2e4").unwrap();
    driver.training.as_mut().unwrap().reply = Some((state, mv));
    driver.training_action(MenuAction::Why).unwrap();
    assert!(driver.board_annotations().iter().all(
        |a| !matches!(&a.primitive, AnnotationPrimitive::Label { run, .. } if run.text == "Threat")
    ));
}
