use chess3d::{
    rules::ChessState,
    teaching::{Course, LessonRunner, Motif},
};
use serde_json::json;

#[test]
fn classified_motifs_use_authored_lesson_tags_and_the_existing_runner() {
    let course = Course::bundled().unwrap();
    for (motif, id) in [
        (Motif::Fork, "tactics.fork"),
        (Motif::Pin, "tactics.pin"),
        (Motif::Skewer, "tactics.skewer"),
        (Motif::HangingPiece, "tactics.hanging"),
        (Motif::Exchange, "tactics.hanging"),
        (Motif::Mate, "mates.queen"),
        (Motif::MateThreat, "mates.back-rank"),
        (Motif::BackRank, "mates.back-rank"),
        (Motif::Check, "rules.check"),
        (Motif::Castling, "openings.reasons"),
        (Motif::MaterialThreat, "practice.together"),
        (Motif::DiscoveredAttack, "practice.together"),
        (Motif::DiscoveredCheck, "practice.together"),
        (Motif::Promotion, "rules.first-moves"),
    ] {
        let lesson = course.related_lesson(motif).unwrap();
        assert_eq!(lesson.id, id);
        assert!(lesson.motifs.contains(&motif));
        assert!(LessonRunner::new(lesson.clone()).is_ok());
    }
}

#[test]
fn course_rejects_unknown_and_duplicate_motif_tags() {
    let mut data: serde_json::Value =
        serde_json::from_str(include_str!("../assets/lessons/first-course.json")).unwrap();
    data["lessons"][0]["motifs"] = json!(["invented"]);
    assert!(Course::from_json(&data.to_string()).is_err());
    data["lessons"][0]["motifs"] = json!(["fork", "fork"]);
    let error = Course::from_json(&data.to_string()).unwrap_err();
    assert!(error.to_string().contains("duplicate motif"));
}

#[test]
fn back_rank_classification_comes_from_the_explained_board() {
    use chess3d::teaching::{Budget, explain_move};
    let state = ChessState::from_fen("6k1/5ppp/8/8/8/8/8/R5K1 w - - 0 1").unwrap();
    let mv = state.parse_uci("a1a8").unwrap();
    let explanation = explain_move(&state, mv, Budget::default()).unwrap();
    assert_eq!(explanation.motif(&state), Some(Motif::BackRank));
}

#[test]
fn lookup_does_not_guess_a_lesson_when_its_motif_tag_is_absent() {
    let mut data: serde_json::Value =
        serde_json::from_str(include_str!("../assets/lessons/first-course.json")).unwrap();
    for lesson in data["lessons"].as_array_mut().unwrap() {
        lesson["motifs"] = json!([]);
    }
    let course = Course::from_json(&data.to_string()).unwrap();
    assert!(course.related_lesson(Motif::Fork).is_none());
}
