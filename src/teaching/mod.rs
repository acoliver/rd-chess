//! CPU-only, original tactical explanations. Geometry proposes a motif; legal
//! continuations verify it. Positional evaluation never supplies its cause.
//!
//! Material values are fixed centipawns (100/320/335/500/950). Verification is
//! finite-horizon minimax with capture/promotion and check-evasion extensions,
//! not a proof of the game's outcome. Exchange facts concern captures on one
//! square only. Every evidence line starts in the caller's original position.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]

use crate::rules::{ChessState, LegalMove, MoveError};
use crate::{PieceKind, Side, Square};

mod board;
mod direct;
mod exchange;
mod motifs;
mod review;
mod templates;
mod verify;

/// One shared node allowance, including alternative search and SEE. Depth is
pub use templates::san_line;

/// total normal plies including the explained move, capped at six. Verification and SEE
/// extensions add at most eight plies; hitting that cap leaves analysis unresolved.
/// Alternative ranking uses the existing search with its 128-ply recursion cap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Budget {
    pub nodes: u64,
    pub depth: u8,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            nodes: 100_000,
            depth: 3,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FactKind {
    Capture {
        attacker: PieceKind,
        victim: PieceKind,
        square: Square,
        balance_change: i32,
    },
    Check,
    Checkmate {
        winner: Side,
    },
    HangingPiece {
        piece: PieceKind,
        square: Square,
        winner: Side,
        gain: i32,
    },
    Exchange {
        gain: i32,
    },
    Fork {
        attacker: Square,
        targets: Vec<Square>,
    },
    Pin {
        attacker: Square,
        front: Square,
        rear: Square,
        rear_piece: PieceKind,
    },
    Skewer {
        attacker: Square,
        front: Square,
        rear: Square,
    },
    DiscoveredAttack {
        attacker: Square,
        target: Square,
        piece: PieceKind,
    },
    DiscoveredCheck {
        attacker: Square,
        piece: PieceKind,
    },
    Promotion {
        into: PieceKind,
    },
    Castling {
        king_side: bool,
    },
    CastlingRightsLost {
        side: Side,
        king_side: bool,
        queen_side: bool,
    },
    MateThreat,
    MaterialThreat {
        gain: i32,
    },
    MissedMate {
        alternative: LegalMove,
    },
    MissedMaterial {
        alternative: LegalMove,
        gain: i32,
    },
}

impl FactKind {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Capture { .. } => "capture",
            Self::Check => "check",
            Self::Checkmate { .. } => "checkmate",
            Self::HangingPiece { .. } => "hanging piece",
            Self::Exchange { .. } => "exchange",
            Self::Fork { .. } => "fork",
            Self::Pin {
                rear_piece: PieceKind::King,
                ..
            } => "absolute pin",
            Self::Pin { .. } => "relative pin",
            Self::Skewer { .. } => "skewer",
            Self::DiscoveredAttack { .. } => "discovered attack",
            Self::DiscoveredCheck { .. } => "discovered check",
            Self::Promotion { .. } => "promotion",
            Self::Castling { .. } => "castling",
            Self::CastlingRightsLost { .. } => "castling rights lost",
            Self::MateThreat => "mate threat",
            Self::MaterialThreat { .. } => "material threat",
            Self::MissedMate { .. } => "missed mate",
            Self::MissedMaterial { .. } => "missed material",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Fact {
    pub kind: FactKind,
    /// Legal UCI-capable moves from the original state, including the explained
    /// move, except missed opportunities which start with the search alternative.
    pub line: Vec<LegalMove>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AnalysisLimits {
    pub budget: Budget,
    pub nodes: u64,
    pub verified_depth: u8,
    pub alternative_depth: u8,
    pub unresolved: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Explanation {
    pub facts: Vec<Fact>,
    /// Fact templates in the same order, followed by any neutral/limit warning.
    pub sentences: Vec<String>,
    /// Best alternative when no motif applies, otherwise the played continuation.
    pub line: Vec<LegalMove>,
    pub limits: AnalysisLimits,
}

/// Rejects stale/illegal moves and counter overflow without mutating the input.
pub fn explain_move(
    state: &ChessState,
    mv: LegalMove,
    budget: Budget,
) -> Result<Explanation, MoveError> {
    let mut after = state.clone();
    after.play(mv)?;
    Ok(review::explain(state, &after, mv, budget))
}

/// Computer replies use exactly the same evidence standards as human moves.
pub fn explain_reply(
    state: &ChessState,
    mv: LegalMove,
    budget: Budget,
) -> Result<Explanation, MoveError> {
    explain_move(state, mv, budget)
}

pub(super) fn add(facts: &mut Vec<Fact>, kind: FactKind, line: &[LegalMove]) {
    facts.push(Fact {
        kind,
        line: line.to_vec(),
    });
}
