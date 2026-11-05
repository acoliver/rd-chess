use super::{Budget, explain_move, explain_reply};
use crate::review::TeachableMoment;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MomentExplanation {
    pub learner: String,
    pub detail: String,
}

pub fn explain_moment(moment: &TeachableMoment) -> MomentExplanation {
    let state = &moment.position_before;
    let played = state.to_san(moment.played).expect("review move");
    let better = state.to_san(moment.better).expect("review alternative");
    let explanation =
        explain_move(state, moment.played, Budget::default()).expect("review move is legal");
    let mut after = state.clone();
    after.play(moment.played).expect("review move is legal");
    let reply =
        moment.refutation.moves.first().map(|&mv| {
            explain_reply(&after, mv, Budget::default()).expect("review reply is legal")
        });
    let learner = reply
        .as_ref()
        .filter(|reply| !reply.facts.is_empty())
        .map_or_else(
            || explanation.sentences.join(" "),
            |reply| format!("After {played}: {}", reply.sentences.join(" ")),
        );
    MomentExplanation {
        detail: format!(
            "Played {played}; consider {better}. {learner}\nReply line: {}.\nSearch estimate: {} ({}). This is a searched continuation, not a proof of a tactical cause.",
            moment.refutation.san.join(" "),
            moment.label,
            moment.loss
        ),
        learner,
    }
}
