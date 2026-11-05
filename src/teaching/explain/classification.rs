use super::{Explanation, Fact, FactKind};
use crate::{PieceKind, Side, rules::ChessState};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Motif {
    Mate,
    MateThreat,
    BackRank,
    Fork,
    Pin,
    Skewer,
    DiscoveredAttack,
    DiscoveredCheck,
    HangingPiece,
    Exchange,
    MaterialThreat,
    Check,
    Castling,
    Promotion,
}

impl Explanation {
    /// Facts already follow learner precedence. A plain capture has no tactical
    /// cause by itself, so it does not select a lesson or explain a review loss.
    pub(super) fn priority(&self, state: &ChessState) -> u8 {
        self.facts
            .iter()
            .filter(|fact| classify(state, fact).is_some())
            .map(|fact| super::presentation::precedence(&fact.kind))
            .min()
            .unwrap_or(u8::MAX)
    }

    pub fn motif(&self, state: &ChessState) -> Option<Motif> {
        self.facts.iter().find_map(|fact| classify(state, fact))
    }
}

fn classify(state: &ChessState, fact: &Fact) -> Option<Motif> {
    Some(match fact.kind {
        FactKind::Checkmate { winner } if back_rank(state, fact, winner) => Motif::BackRank,
        FactKind::Checkmate { .. } | FactKind::MissedMate { .. } => Motif::Mate,
        FactKind::MateThreat => Motif::MateThreat,
        FactKind::Fork { .. } => Motif::Fork,
        FactKind::Pin { .. } => Motif::Pin,
        FactKind::Skewer { .. } => Motif::Skewer,
        FactKind::DiscoveredAttack { .. } => Motif::DiscoveredAttack,
        FactKind::DiscoveredCheck { .. } => Motif::DiscoveredCheck,
        FactKind::HangingPiece { .. } => Motif::HangingPiece,
        FactKind::Exchange { .. } => Motif::Exchange,
        FactKind::MaterialThreat { .. } | FactKind::MissedMaterial { .. } => Motif::MaterialThreat,
        FactKind::Check => Motif::Check,
        FactKind::Castling { .. } | FactKind::CastlingRightsLost { .. } => Motif::Castling,
        FactKind::Promotion { .. } => Motif::Promotion,
        FactKind::Capture { .. } => return None,
    })
}

fn back_rank(state: &ChessState, fact: &Fact, winner: Side) -> bool {
    let mut after = state.clone();
    for &mv in &fact.line {
        after.play(mv).expect("verified explanation line");
    }
    let rank = if winner == Side::Ivory { 7 } else { 0 };
    let king = super::board::squares()
        .find(|&sq| {
            after
                .piece_at(sq)
                .is_some_and(|(side, kind)| side != winner && kind == PieceKind::King)
        })
        .expect("legal position has a king");
    king.rank == rank
        && fact.line.last().is_some_and(|mv| {
            mv.to().rank == rank
                && matches!(
                    after.piece_at(mv.to()),
                    Some((_, PieceKind::Rook | PieceKind::Queen))
                )
                && super::board::attacks(&after, mv.to(), king)
        })
}
