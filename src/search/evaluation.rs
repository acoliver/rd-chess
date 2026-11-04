//! Original, deliberately small evaluation constants. Material MG/EG values are
//! pawn 100/120, knight 320/300, bishop 335/340, rook 500/520, queen 950/930.
//! Placement tables are generated from rank advance and distance from the centre:
//! pawns gain 6/12 per rank; minors gain 5/4 per centre step; rooks gain 2/3 per
//! rank; queens gain 2/3 per centre step; kings avoid the centre in MG (-8) and
//! approach it in EG (+10). These are authored heuristics, not fitted ratings.

use super::features::Features;
use crate::rules::ChessState;
use crate::{PieceKind, Side};

/// One separately inspectable feature, in white-minus-black centipawns.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Term {
    pub middlegame: i32,
    pub endgame: i32,
}

impl std::ops::Neg for Term {
    type Output = Self;
    fn neg(self) -> Self {
        Self {
            middlegame: -self.middlegame,
            endgame: -self.endgame,
        }
    }
}

impl Term {
    pub(super) fn add(&mut self, value: Self, sign: i32) {
        self.middlegame += value.middlegame * sign;
        self.endgame += value.endgame * sign;
    }

    fn tapered(self, phase: i32) -> i64 {
        i64::from(self.middlegame * phase + self.endgame * (24 - phase)) / 24
    }
}

/// Attention percentages, with 100 meaning full attention. No term is hidden in
/// the search. Large caller-supplied weights are accumulated in i64 and clamped.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Weights {
    pub material: u16,
    pub placement: u16,
    pub mobility: u16,
    pub king_safety: u16,
    pub pawns: u16,
}

impl Weights {
    pub const BALANCED: Self = Self {
        material: 100,
        placement: 100,
        mobility: 100,
        king_safety: 100,
        pawns: 100,
    };
}

/// Features are white-minus-black; `total` tapers at phase 0 (endgame) to 24
/// (initial material). Queens count 4, rooks 2, and each minor 1 phase unit.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct EvaluationBreakdown {
    pub material: Term,
    pub placement: Term,
    pub mobility: Term,
    pub king_safety: Term,
    pub pawns: Term,
    pub phase: i32,
}

impl EvaluationBreakdown {
    pub fn total(self, weights: Weights) -> i32 {
        let terms = [
            (self.material, weights.material),
            (self.placement, weights.placement),
            (self.mobility, weights.mobility),
            (self.king_safety, weights.king_safety),
            (self.pawns, weights.pawns),
        ];
        let sum: i64 = terms
            .into_iter()
            .map(|(term, weight)| term.tapered(self.phase) * i64::from(weight) / 100)
            .sum();
        i32::try_from(sum.clamp(-100_000, 100_000)).expect("clamped score")
    }
}

pub fn evaluate(state: &ChessState) -> EvaluationBreakdown {
    let features = Features::new(state);
    let mut result = EvaluationBreakdown::default();
    for (square, side, kind) in features.men() {
        let sign = if side == Side::Ivory { 1 } else { -1 };
        result.material.add(material(kind), sign);
        result
            .placement
            .add(super::placement::at(kind, square, side), sign);
        result.mobility.add(features.mobility(square, kind), sign);
        match kind {
            PieceKind::Pawn => result
                .pawns
                .add(features.pawn_structure(square, side), sign),
            PieceKind::King => result
                .king_safety
                .add(features.king_safety(square, side), sign),
            _ => (),
        }
        result.phase += match kind {
            PieceKind::Knight | PieceKind::Bishop => 1,
            PieceKind::Rook => 2,
            PieceKind::Queen => 4,
            _ => 0,
        };
    }
    result.phase = result.phase.min(24);
    result
}

pub(super) fn material(kind: PieceKind) -> Term {
    let (middlegame, endgame) = match kind {
        PieceKind::Pawn => (100, 120),
        PieceKind::Knight => (320, 300),
        PieceKind::Bishop => (335, 340),
        PieceKind::Rook => (500, 520),
        PieceKind::Queen => (950, 930),
        PieceKind::King => (0, 0),
    };
    Term {
        middlegame,
        endgame,
    }
}
