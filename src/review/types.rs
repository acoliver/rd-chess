use super::{ChessState, LegalMove, Score};
use std::fmt;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Label {
    Best,
    Good,
    Inaccuracy,
    Mistake,
    Blunder,
    MissedMate,
    AllowedMate,
    Unclear,
}

impl Label {
    pub fn is_mistake(self) -> bool {
        matches!(
            self,
            Self::Mistake | Self::Blunder | Self::MissedMate | Self::AllowedMate
        )
    }
}

impl fmt::Display for Label {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Best => "best",
            Self::Good => "good",
            Self::Inaccuracy => "inaccuracy",
            Self::Mistake => "mistake",
            Self::Blunder => "blunder",
            Self::MissedMate => "missed mate",
            Self::AllowedMate => "allowed mate",
            Self::Unclear => "unclear",
        })
    }
}

/// Ord ranks allowed mate, missed mate, mate-distance change, then centipawns.
/// Mate-distance changes are uncertain and never selected as teachable moments.
/// No artificial centipawn value is assigned to a mate.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum Loss {
    Centipawns(i32),
    MateDistance(u16),
    MissedMate,
    AllowedMate,
}

impl fmt::Display for Loss {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Centipawns(cp) => write!(formatter, "{cp} cp"),
            Self::MateDistance(plies) => write!(formatter, "mate distance {plies} plies"),
            Self::MissedMate => formatter.write_str("missed mate"),
            Self::AllowedMate => formatter.write_str("allowed mate"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Uncertainty {
    Shallow,
    UnequalDepth,
    CloseEvaluation,
    MateDistance,
    SearchDisagreement,
    NoContinuation,
}

/// Moves and SAN are replayed through `ChessState` before publication.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewLine {
    pub moves: Vec<LegalMove>,
    pub san: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlyReview {
    /// One-based index in the supplied move record, including for black-to-move FENs.
    pub ply: usize,
    pub position_before: ChessState,
    pub played: LegalMove,
    pub played_san: String,
    /// None without a completed iteration, including an available draw claim.
    pub best_move: Option<LegalMove>,
    pub best_line: ReviewLine,
    /// Centipawns before and after are both relative to the mover, not the next
    /// side to move. Mate winners are absolute; after distances start after play.
    pub before: Score,
    pub after: Score,
    pub loss: Loss,
    pub label: Label,
    pub uncertainty: Option<Uncertainty>,
    pub best_depth: u8,
    pub played_depth: u8,
    pub best_nodes: u64,
    pub played_nodes: u64,
    /// Starts AFTER the recorded move, with the opponent's reply.
    pub refutation: ReviewLine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TeachableMoment {
    pub ply: usize,
    /// Exact state including repetition history, suitable for Try again.
    pub position_before: ChessState,
    pub played: LegalMove,
    pub better: LegalMove,
    pub label: Label,
    pub loss: Loss,
    pub refutation: ReviewLine,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReviewReport {
    pub analysis_version: u32,
    pub nodes_per_search: u64,
    pub depth_limit: Option<u8>,
    pub plies: Vec<PlyReview>,
    pub moments: Vec<TeachableMoment>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Progress {
    pub completed: usize,
    pub total: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReviewStep {
    Advanced,
    Cancelled,
    Complete,
}
