//! Grounded own-game feedback from rules-verified review continuations.
use crate::review::TeachableMoment;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Explanation {
    pub learner: String,
    pub detail: String,
}

pub fn explain(moment: &TeachableMoment) -> Explanation {
    let played = moment
        .position_before
        .to_san(moment.played)
        .expect("review move");
    let better = moment
        .position_before
        .to_san(moment.better)
        .expect("review alternative");
    let learner = moment.refutation.san.first().map_or_else(
        || format!("Compare {played} with the searched alternative {better}."),
        |reply| format!("After {played}, the opponent can reply {reply}."),
    );
    let line = moment.refutation.san.join(" ");
    Explanation {
        detail: format!(
            "Played {played}; consider {better}. {learner}\nReply line: {line}.\nSearch estimate: {} ({}). This is a searched continuation, not a proof of a tactical cause.",
            moment.label, moment.loss
        ),
        learner,
    }
}
