//! Learner precedence: mate > winning tactic (fork/pin/skewer/discovered) >
//! hanging/exchange > threat > check > castling/promotion notes.
//! Precedence orders facts, not independent evidence away. A stronger fact
//! subsumes a weaker one only on the same line (or its direct-move prefix):
//! mate covers that line, a winning motif covers its material threat, and a
//! fork of the king or discovered check covers the direct check. Capture and
//! promotion describe distinct board changes and remain unless mate covers them.

use super::{Fact, FactKind};
use crate::PieceKind;
use crate::rules::ChessState;

pub(super) fn select(state: &ChessState, mut facts: Vec<Fact>) -> Vec<Fact> {
    facts.sort_by_key(|fact| precedence(&fact.kind));
    let mut selected: Vec<Fact> = Vec::new();
    for fact in facts {
        if !selected.iter().any(|strong| subsumes(state, strong, &fact)) {
            selected.push(fact);
        }
    }
    selected
}

fn precedence(kind: &FactKind) -> u8 {
    match kind {
        FactKind::Checkmate { .. } | FactKind::MateThreat | FactKind::MissedMate { .. } => 0,
        FactKind::Fork { .. }
        | FactKind::Pin { .. }
        | FactKind::Skewer { .. }
        | FactKind::DiscoveredAttack { .. }
        | FactKind::DiscoveredCheck { .. } => 1,
        FactKind::HangingPiece { .. } | FactKind::Exchange { .. } | FactKind::Capture { .. } => 2,
        FactKind::MaterialThreat { .. } | FactKind::MissedMaterial { .. } => 3,
        FactKind::Check => 4,
        FactKind::Castling { .. }
        | FactKind::Promotion { .. }
        | FactKind::CastlingRightsLost { .. } => 5,
    }
}

fn subsumes(state: &ChessState, strong: &Fact, weak: &Fact) -> bool {
    if !strong.line.starts_with(&weak.line) {
        return false;
    }
    match &strong.kind {
        FactKind::Checkmate { .. } | FactKind::MateThreat | FactKind::MissedMate { .. } => true,
        FactKind::Fork { targets, .. } => {
            matches!(weak.kind, FactKind::MaterialThreat { .. })
                || (weak.kind == FactKind::Check
                    && targets.iter().any(|&sq| {
                        state
                            .piece_at(sq)
                            .is_some_and(|(_, kind)| kind == PieceKind::King)
                    }))
        }
        FactKind::Pin { .. } | FactKind::Skewer { .. } | FactKind::DiscoveredAttack { .. } => {
            matches!(weak.kind, FactKind::MaterialThreat { .. })
        }
        FactKind::DiscoveredCheck { .. } => weak.kind == FactKind::Check,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::LegalMove;
    use crate::{Side, Square};

    fn fact(kind: FactKind, line: &[LegalMove]) -> Fact {
        Fact {
            kind,
            line: line.to_vec(),
        }
    }

    #[test]
    fn precedence_is_explicit_and_independent_lines_survive() {
        let state = ChessState::initial();
        let a = [state.parse_uci("e2e4").unwrap()];
        let b = [state.parse_uci("d2d4").unwrap()];
        let square = Square::new(0, 0).unwrap();
        let ordered = vec![
            fact(
                FactKind::Checkmate {
                    winner: Side::Ivory,
                },
                &b,
            ),
            fact(
                FactKind::Pin {
                    attacker: square,
                    front: square,
                    rear: square,
                    rear_piece: PieceKind::King,
                },
                &a,
            ),
            fact(FactKind::Exchange { gain: 100 }, &a),
            fact(
                FactKind::MissedMaterial {
                    alternative: b[0],
                    gain: 100,
                },
                &a,
            ),
            fact(FactKind::Check, &a),
            fact(
                FactKind::Promotion {
                    into: PieceKind::Queen,
                },
                &a,
            ),
        ];
        let mut reversed = ordered.clone();
        reversed.reverse();
        assert_eq!(select(&state, reversed), ordered);
    }

    #[test]
    fn mate_subsumes_its_line_but_not_an_alternative() {
        let state = ChessState::initial();
        let a = [state.parse_uci("e2e4").unwrap()];
        let b = [state.parse_uci("d2d4").unwrap()];
        let mate = fact(FactKind::MateThreat, &a);
        let other = fact(FactKind::MaterialThreat { gain: 100 }, &b);
        assert_eq!(
            select(
                &state,
                vec![fact(FactKind::Check, &a), other.clone(), mate.clone()]
            ),
            vec![mate, other]
        );
    }
}
