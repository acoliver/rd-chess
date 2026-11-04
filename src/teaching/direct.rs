use super::board::{material, victim};
use super::{Fact, FactKind, add};
use crate::rules::{ChessState, GameStatus, LegalMove};
use crate::{PieceKind, Side};

pub(super) fn facts(state: &ChessState, after: &ChessState, mv: LegalMove) -> Vec<Fact> {
    let mut result = Vec::new();
    let mover = state.piece_at(mv.from()).expect("validated mover").1;
    let line = [mv];
    if let Some((square, victim)) = victim(state, mv) {
        add(
            &mut result,
            FactKind::Capture {
                attacker: mover,
                victim,
                square,
                balance_change: material(after, state.side_to_move())
                    - material(state, state.side_to_move()),
            },
            &line,
        );
    }
    if let GameStatus::Checkmate { winner } = after.status() {
        add(&mut result, FactKind::Checkmate { winner }, &line);
    } else if after.in_check() {
        add(&mut result, FactKind::Check, &line);
    }
    if mv.promotion().is_some() {
        add(
            &mut result,
            FactKind::Promotion {
                into: after.piece_at(mv.to()).expect("promoted piece").1,
            },
            &line,
        );
    }
    if mover == PieceKind::King && mv.from().file.abs_diff(mv.to().file) == 2 {
        add(
            &mut result,
            FactKind::Castling {
                king_side: mv.to().file == 6,
            },
            &line,
        );
    }
    rights(state, after, &line, &mut result);
    result
}

fn rights(state: &ChessState, after: &ChessState, line: &[LegalMove], facts: &mut Vec<Fact>) {
    for side in [Side::Ivory, Side::Onyx] {
        let before = state.castling_rights();
        let next = after.castling_rights();
        let king_side = before.king_side(side) && !next.king_side(side);
        let queen_side = before.queen_side(side) && !next.queen_side(side);
        if king_side || queen_side {
            add(
                facts,
                FactKind::CastlingRightsLost {
                    side,
                    king_side,
                    queen_side,
                },
                line,
            );
        }
    }
}
