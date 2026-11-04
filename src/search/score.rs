use crate::Side;
use std::cmp::Reverse;

/// Centipawns are relative to the side to move at the reported position.
/// Mate has an absolute winner and a distance in plies, never a pawn value.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Score {
    Centipawns(i32),
    Mate { winner: Side, plies: u16 },
}

/// Ordered search bounds keep mate distances separate from centipawn arithmetic.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Value {
    Bottom,
    Loss(u16),
    Pawns(i32),
    Win(Reverse<u16>),
    Top,
}

impl Value {
    pub fn reversed(self) -> Self {
        match self {
            Self::Bottom => Self::Top,
            Self::Loss(plies) => Self::Win(Reverse(plies)),
            Self::Pawns(value) => Self::Pawns(-value),
            Self::Win(Reverse(plies)) => Self::Loss(plies),
            Self::Top => Self::Bottom,
        }
    }
}

impl Score {
    pub(super) fn value(self, perspective: Side) -> Value {
        match self {
            Self::Centipawns(value) => Value::Pawns(value),
            Self::Mate { winner, plies } if winner == perspective => Value::Win(Reverse(plies)),
            Self::Mate { plies, .. } => Value::Loss(plies),
        }
    }

    pub(super) fn reversed(self) -> Self {
        match self {
            Self::Centipawns(value) => Self::Centipawns(-value),
            mate @ Self::Mate { .. } => mate,
        }
    }

    pub(super) fn stored(self, ply: u16) -> Self {
        match self {
            Self::Mate { winner, plies } => Self::Mate {
                winner,
                plies: plies.checked_sub(ply).expect("mate follows current node"),
            },
            value @ Self::Centipawns(_) => value,
        }
    }

    pub(super) fn restored(self, ply: u16) -> Self {
        match self {
            Self::Mate { winner, plies } => Self::Mate {
                winner,
                plies: plies + ply,
            },
            value @ Self::Centipawns(_) => value,
        }
    }
}
