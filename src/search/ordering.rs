use super::evaluation::material;
use super::features::index;
use crate::rules::{ChessState, LegalMove, PromotionPiece};
use crate::{PieceKind, Side};
use std::cmp::Reverse;

pub(super) struct Ordering {
    killers: [[Option<LegalMove>; 2]; 128],
    history: Vec<[[i32; 64]; 64]>,
}

pub(super) fn captured(state: &ChessState, chess_move: LegalMove) -> Option<PieceKind> {
    state
        .piece_at(chess_move.to())
        .map(|(_, kind)| kind)
        .or_else(|| {
            (state
                .piece_at(chess_move.from())
                .is_some_and(|(_, kind)| kind == PieceKind::Pawn)
                && chess_move.from().file != chess_move.to().file)
                .then_some(PieceKind::Pawn)
        })
}

pub(super) fn tactical(state: &ChessState, chess_move: LegalMove) -> bool {
    chess_move.promotion().is_some() || captured(state, chess_move).is_some()
}

impl Ordering {
    pub fn new() -> Self {
        Self {
            killers: [[None; 2]; 128],
            history: vec![[[0; 64]; 64]; 2],
        }
    }

    pub fn sort(
        &self,
        state: &ChessState,
        moves: &mut [LegalMove],
        tt: Option<LegalMove>,
        ply: u16,
    ) {
        moves.sort_by_cached_key(|&chess_move| Reverse(self.priority(state, chess_move, tt, ply)));
    }

    fn priority(
        &self,
        state: &ChessState,
        chess_move: LegalMove,
        tt: Option<LegalMove>,
        ply: u16,
    ) -> (bool, i32, i32, i32) {
        let mover = state
            .piece_at(chess_move.from())
            .expect("generated mover")
            .1;
        let capture = captured(state, chess_move).map_or(0, |victim| {
            10_000 + material(victim).middlegame * 16 - material(mover).middlegame
        });
        let promotion = match chess_move.promotion() {
            Some(PromotionPiece::Queen) => 900,
            Some(PromotionPiece::Rook) => 500,
            Some(PromotionPiece::Bishop | PromotionPiece::Knight) => 300,
            None => 0,
        };
        let killers = self.killers[usize::from(ply)];
        let killer = if killers[0] == Some(chess_move) {
            2
        } else {
            i32::from(killers[1] == Some(chess_move))
        };
        let side = usize::from(state.side_to_move() == Side::Onyx);
        (
            Some(chess_move) == tt,
            capture + promotion,
            killer,
            self.history[side][index(chess_move.from())][index(chess_move.to())],
        )
    }

    pub fn cutoff(&mut self, state: &ChessState, chess_move: LegalMove, ply: u16, depth: u8) {
        if tactical(state, chess_move) {
            return;
        }
        let killers = &mut self.killers[usize::from(ply)];
        if killers[0] != Some(chess_move) {
            killers[1] = killers[0];
            killers[0] = Some(chess_move);
        }
        let side = usize::from(state.side_to_move() == Side::Onyx);
        let history = &mut self.history[side][index(chess_move.from())][index(chess_move.to())];
        *history = history.saturating_add(i32::from(depth).pow(2));
    }
}
