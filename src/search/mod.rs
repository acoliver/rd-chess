//! Original CPU-only chess search. No external chess-engine source is used.
//!
//! Node limits count every visited root, normal and quiescence node exactly;
//! cancellation is checked before each node. Move generation, evaluation, TT
//! probes and the policy's finite immediate-queen-safety scan are not extra nodes.
//! An interrupted iteration is discarded. With no completed iteration the result
//! explicitly has depth zero, a static score and the first legal fallback move.
//! Claims are taken immediately; analysis does not decline draws to seek a win.
//! Search never mutates the caller's state. Host threads and stale-request
//! rejection are deliberately outside this module.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]

use crate::rules::{ChessState, LegalMove};
use std::sync::atomic::AtomicBool;
use table::Table;
use tree::Context;

mod evaluation;
mod features;
mod ordering;
mod placement;
mod policy;
mod profiles;
mod score;
mod table;
mod tree;

// A generated move failing legality is a search/rules bug, not cancellation.
// Counter exhaustion is supported FEN input and cannot be advanced by the core.
fn play_generated(state: &mut ChessState, chess_move: LegalMove) -> Option<()> {
    match state.play(chess_move) {
        Ok(()) => Some(()),
        Err(crate::rules::MoveError::CounterOverflow) => None,
        Err(error) => panic!("generated search move rejected: {error}"),
    }
}

pub use evaluation::{EvaluationBreakdown, Term, Weights, evaluate};
pub use policy::{choose_move, choose_move_cancellable};
pub use profiles::{
    ELLIS, IMANI, LADDER, MIRA, NIA, OpponentProfile, PIP, ROWAN, SOREN, Style, TOMAS,
};
pub use score::Score;

/// Depth is capped at 64 normal plies. Total normal plus quiescence recursion
/// is capped at 128; reaching it aborts the iteration rather than inventing a
/// quiet evaluation while in check. `None` deepens until the node budget ends.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    pub nodes: u64,
    pub depth: Option<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Candidate {
    pub score: Score,
    pub pv: Vec<LegalMove>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Analysis {
    pub completed_depth: u8,
    pub nodes: u64,
    pub score: Score,
    pub pv: Vec<LegalMove>,
    /// Exact top-N root scores from the same completed iteration, sorted best
    /// first. Inferior bound-only moves are omitted. Empty at depth zero.
    pub candidates: Vec<Candidate>,
}

#[derive(Clone)]
struct Line {
    score: Score,
    pv: Vec<LegalMove>,
}

/// Fixed slot count, chosen by the caller (zero disables TT). Each entry holds
/// at most 128 PV moves; storage cannot grow with the number of visited nodes.
/// Same-age collisions retain the deeper entry; a new search age replaces old
/// entries. Bounds validate the reversible draw-history fingerprint as well as
/// the existing Zobrist key. The table is private to this evaluator's weights.
pub struct Search {
    table: Table,
    weights: Weights,
    width: usize,
}

impl Search {
    pub fn new(capacity: usize) -> Self {
        Self::weighted(capacity, Weights::BALANCED)
    }

    pub fn weighted(capacity: usize, weights: Weights) -> Self {
        Self {
            table: Table::new(capacity),
            weights,
            width: 1,
        }
    }

    /// Request exact scores for the top N legal roots (default one). Larger
    /// candidate sets spend more nodes; zero is treated as one.
    #[must_use]
    pub fn candidates(mut self, width: usize) -> Self {
        self.width = width.max(1);
        self
    }

    pub fn analyze(&mut self, state: &ChessState, limits: Limits, cancel: &AtomicBool) -> Analysis {
        self.analyze_progress(state, limits, cancel, |_| {})
    }

    /// Analyze one legal root with the same evaluator and node accounting as
    /// `analyze`. Scores and mate distances still refer to the supplied position.
    /// This supplies an exact played-move comparison without opponent policy.
    pub fn analyze_move(
        &mut self,
        state: &ChessState,
        chess_move: LegalMove,
        limits: Limits,
        cancel: &AtomicBool,
    ) -> Result<Analysis, crate::rules::MoveError> {
        if !state.legal_moves().contains(&chess_move) {
            return Err(crate::rules::MoveError::IllegalMove);
        }
        let mut result = self.analyze_moves(
            state,
            limits,
            &|| cancel.load(std::sync::atomic::Ordering::Relaxed),
            &[chess_move],
        );
        if result.completed_depth == 0 && !result.pv.is_empty() {
            let mut child = state.clone();
            child.play(chess_move)?;
            let fallback = self.analyze(
                &child,
                Limits {
                    nodes: 0,
                    depth: Some(0),
                },
                cancel,
            );
            result.score = match fallback.score {
                Score::Mate { winner, plies } => Score::Mate {
                    winner,
                    plies: plies + 1,
                },
                cp @ Score::Centipawns(_) => cp.reversed(),
            };
            result.pv = vec![chess_move];
        }
        Ok(result)
    }

    /// Progress is published only for complete iterations. The callback may set
    /// the cancellation flag; the last published iteration is still returned.
    pub fn analyze_progress(
        &mut self,
        state: &ChessState,
        limits: Limits,
        cancel: &AtomicBool,
        completed: impl FnMut(&Analysis),
    ) -> Analysis {
        self.run(
            state,
            limits,
            &|| cancel.load(std::sync::atomic::Ordering::Relaxed),
            completed,
            None,
        )
    }

    fn analyze_moves(
        &mut self,
        state: &ChessState,
        limits: Limits,
        cancel: &dyn Fn() -> bool,
        moves: &[LegalMove],
    ) -> Analysis {
        self.run(state, limits, cancel, |_| {}, Some(moves))
    }

    fn run(
        &mut self,
        state: &ChessState,
        limits: Limits,
        cancel: &dyn Fn() -> bool,
        mut completed: impl FnMut(&Analysis),
        roots: Option<&[LegalMove]>,
    ) -> Analysis {
        self.table.age = self.table.age.wrapping_add(1);
        let mut context = Context::new(&mut self.table, self.weights, limits, cancel);
        let mut result = context.fallback(state);
        if Context::terminal(state, 0).is_some() {
            return result;
        }
        let mut position = state.clone();
        for depth in 1..=limits.depth.unwrap_or(64).min(64) {
            let Some(candidates) = context.iteration(&mut position, depth, self.width, roots)
            else {
                break;
            };
            let best = &candidates[0];
            result = Analysis {
                completed_depth: depth,
                nodes: context.nodes,
                score: best.score,
                pv: best.pv.clone(),
                candidates,
            };
            completed(&result);
        }
        result.nodes = context.nodes;
        result
    }
}

#[cfg(test)]
mod tests;
