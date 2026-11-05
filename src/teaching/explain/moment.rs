use super::{Budget, Motif, explain_move, explain_reply};
use crate::review::{Label, TeachableMoment};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MomentExplanation {
    pub learner: String,
    pub detail: String,
    pub motif: Option<Motif>,
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
    let (sentences, motif) = reply
        .as_ref()
        .filter(|reply| reply.priority(&after) < explanation.priority(state))
        .map_or_else(
            || (explanation.sentences.join(" "), explanation.motif(state)),
            |reply| {
                (
                    format!("After {played}: {}", reply.sentences.join(" ")),
                    reply.motif(&after),
                )
            },
        );
    let learner = if motif.is_none() || moment.label == Label::Unclear {
        format!(
            "Compare {played} with {better}. This is a searched continuation, not a proof of a tactical cause."
        )
    } else {
        sentences
    };
    let learner = format!(
        "{learner} Searched reply: {}.",
        moment.refutation.san.join(" ")
    );
    MomentExplanation {
        detail: format!(
            "Played {played}; consider {better}. {learner}\nReply line: {}.\nSearch estimate: {} ({}). This is a searched continuation, not a proof of a tactical cause.",
            moment.refutation.san.join(" "),
            moment.label,
            moment.loss
        ),
        learner,
        motif: (moment.label != Label::Unclear).then_some(motif).flatten(),
    }
}

#[cfg(test)]
mod tests;
