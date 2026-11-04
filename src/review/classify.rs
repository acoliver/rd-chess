use super::{GAP_MARGIN_CP, Label, Loss, MIN_DEPTH, THRESHOLD_MARGIN_CP, Uncertainty};
use crate::Side;
use crate::search::{Analysis, Score};

pub(super) fn loss(before: Score, played: Score, mover: Side) -> Loss {
    match (before, played) {
        (
            Score::Mate {
                winner,
                plies: first,
            },
            Score::Mate {
                winner: other,
                plies: next,
            },
        ) if winner == other => Loss::MateDistance(if winner == mover {
            next.saturating_sub(first)
        } else {
            first.saturating_sub(next)
        }),
        (_, Score::Mate { winner, .. }) if winner != mover => Loss::AllowedMate,
        (Score::Mate { winner, .. }, _) if winner == mover => Loss::MissedMate,
        (Score::Centipawns(before), Score::Centipawns(after)) => {
            Loss::Centipawns(before.saturating_sub(after).max(0))
        }
        _ => Loss::MateDistance(0),
    }
}

pub(super) fn label(
    best: &Analysis,
    played: &Analysis,
    loss: Loss,
    same_move: bool,
) -> (Label, Option<Uncertainty>) {
    if best.completed_depth < MIN_DEPTH || played.completed_depth < MIN_DEPTH {
        return unclear(Uncertainty::Shallow);
    }
    if best.completed_depth != played.completed_depth {
        return unclear(Uncertainty::UnequalDepth);
    }
    if best.pv.is_empty() || played.pv.is_empty() {
        return unclear(Uncertainty::NoContinuation);
    }
    if same_move {
        return (Label::Best, None);
    }
    match loss {
        Loss::AllowedMate => (Label::AllowedMate, None),
        Loss::MissedMate => (Label::MissedMate, None),
        Loss::MateDistance(_) => unclear(Uncertainty::MateDistance),
        Loss::Centipawns(cp) => {
            if let (Score::Centipawns(before), Score::Centipawns(after)) =
                (best.score, played.score)
                && after > before
            {
                return unclear(Uncertainty::SearchDisagreement);
            }
            let label = centipawns(cp);
            (
                label,
                (label == Label::Unclear).then_some(Uncertainty::CloseEvaluation),
            )
        }
    }
}

fn unclear(reason: Uncertainty) -> (Label, Option<Uncertainty>) {
    (Label::Unclear, Some(reason))
}

pub(super) fn centipawns(cp: i32) -> Label {
    if cp <= GAP_MARGIN_CP
        || [50, 150, 300]
            .iter()
            .any(|&boundary| cp.abs_diff(boundary) <= THRESHOLD_MARGIN_CP.unsigned_abs())
    {
        Label::Unclear
    } else if cp < 50 {
        Label::Good
    } else if cp < 150 {
        Label::Inaccuracy
    } else if cp < 300 {
        Label::Mistake
    } else {
        Label::Blunder
    }
}
