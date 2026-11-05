#[test]
fn learner_can_explain_the_opponents_fork_with_reply_templates() {
    let moment = moment(
        "4k3/8/8/8/1n6/8/8/3QK3 w - - 0 1",
        "d1a1",
        "d1b3",
        &["b4c2"],
    );
    let mut after = moment.position_before.clone();
    after.play(moment.played).unwrap();
    let reply = explain_reply(&after, moment.refutation.moves[0], Budget::default()).unwrap();
    let explanation = explain_moment(&moment);
    assert_eq!(explanation.motif, Some(Motif::Fork));
    assert!(explanation.learner.contains(&reply.sentences[0]));
    assert!(explanation.learner.contains("forks"));
}

use super::*;
use crate::{
    Side,
    review::{Label, Loss, ReviewLine},
    rules::ChessState,
};

fn moment(fen: &str, played: &str, better: &str, reply: &[&str]) -> TeachableMoment {
    let state = ChessState::from_fen(fen).unwrap();
    let played = state.parse_uci(played).unwrap();
    let better = state.parse_uci(better).unwrap();
    let mut after = state.clone();
    after.play(played).unwrap();
    let mut moves = Vec::new();
    let mut san = Vec::new();
    for uci in reply {
        let mv = after.parse_uci(uci).unwrap();
        san.push(after.to_san(mv).unwrap());
        moves.push(mv);
        after.play(mv).unwrap();
    }
    TeachableMoment {
        ply: 1,
        position_before: state,
        played,
        better,
        loss: Loss::Centipawns(900),
        label: Label::Blunder,
        refutation: ReviewLine { moves, san },
    }
}

#[test]
fn learner_uses_real_hanging_piece_explanation_and_legal_refutation() {
    let moment = moment("4k3/8/8/8/r7/8/8/3QK3 w - - 0 1", "d1d4", "d1a4", &["a4d4"]);
    let explanation = explain_moment(&moment);
    let real = explain_move(&moment.position_before, moment.played, Budget::default()).unwrap();
    assert_eq!(explanation.motif, Some(Motif::HangingPiece));
    assert!(explanation.learner.contains(&real.sentences[0]));
    assert!(explanation.learner.contains("queen"));
    assert!(explanation.learner.contains("Rxd4"));
    assert!(
        explanation
            .detail
            .contains("searched continuation, not a proof")
    );
    assert_eq!(moment.position_before.side_to_move(), Side::Ivory);
}

#[test]
fn uncaused_and_unclear_moments_keep_neutral_wording_and_no_lesson_motif() {
    let mut quiet = moment(crate::rules::INITIAL_FEN, "g1f3", "e2e4", &[]);
    let explanation = explain_moment(&quiet);
    assert!(
        explanation
            .learner
            .contains("searched continuation, not a proof")
    );
    assert_eq!(explanation.motif, None);
    quiet.label = Label::Unclear;
    assert_eq!(explain_moment(&quiet).motif, None);
}
