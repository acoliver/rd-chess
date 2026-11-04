//! CPU-only own-game review. See `README.md` for budgets and label thresholds.
//!
//! Each advance analyzes one committed ply without changing the recorded game.
//! Cancellation is checked between plies, not during a ply; no partially analyzed
//! row is published. Fresh neutral searches make resumed reports reproducible.
//! The caller supplies the player's side and ends the record at resignation,
//! agreement or a claim; a terminal board is not required (short prefixes are
//! useful for inspection). Moves after automatic outcomes are rejected.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]

use crate::Side;
use crate::rules::{ChessState, GameStatus, LegalMove, MoveError};
use crate::search::{Limits, Score, Search};
use std::sync::atomic::{AtomicBool, Ordering};

mod analysis;
mod classify;
mod types;
pub use types::*;

pub const ANALYSIS_VERSION: u32 = 1;
/// Twice Soren's node allowance per search, without policy noise or depth cap.
pub const ANALYSIS_NODES: u64 = 240_000;
pub const MIN_DEPTH: u8 = 2;
pub const GAP_MARGIN_CP: i32 = 20;
pub const THRESHOLD_MARGIN_CP: i32 = 10;
const TABLE_SLOTS: usize = 16_384;

#[derive(Clone, Copy, Debug)]
pub struct ReviewConfig {
    pub limits: Limits,
    pub max_moments: usize,
}

impl Default for ReviewConfig {
    fn default() -> Self {
        Self {
            limits: Limits {
                nodes: ANALYSIS_NODES,
                depth: None,
            },
            max_moments: 3,
        }
    }
}

/// Immutable report data remains accessible during cancellation and progress.
/// Construction replays and validates the entire supplied record before search.
pub struct Review {
    position: ChessState,
    moves: Vec<LegalMove>,
    player: Side,
    config: ReviewConfig,
    report: ReviewReport,
}

impl Review {
    pub fn new(
        initial: ChessState,
        moves: Vec<LegalMove>,
        player: Side,
        config: ReviewConfig,
    ) -> Result<Self, MoveError> {
        let mut check = initial.clone();
        for &chess_move in &moves {
            if !matches!(check.status(), GameStatus::Ongoing { .. }) {
                return Err(MoveError::IllegalMove);
            }
            check.play(chess_move)?;
        }
        Ok(Self {
            position: initial,
            moves,
            player,
            config,
            report: ReviewReport {
                analysis_version: ANALYSIS_VERSION,
                nodes_per_search: config.limits.nodes,
                depth_limit: config.limits.depth,
                plies: Vec::new(),
                moments: Vec::new(),
            },
        })
    }

    pub fn report(&self) -> &ReviewReport {
        &self.report
    }

    pub fn progress(&self) -> Progress {
        Progress {
            completed: self.report.plies.len(),
            total: self.moves.len(),
        }
    }

    /// A successful call commits exactly one row. Set cancellation before the
    /// next call to pause; clear it to resume. Completion takes precedence.
    pub fn advance(&mut self, cancel: &AtomicBool) -> Result<ReviewStep, MoveError> {
        let index = self.report.plies.len();
        let Some(&played) = self.moves.get(index) else {
            return Ok(ReviewStep::Complete);
        };
        if cancel.load(Ordering::Relaxed) {
            return Ok(ReviewStep::Cancelled);
        }
        let row = analysis::analyze_ply(&self.position, played, self.config.limits, index)?;
        self.position.play(played)?;
        self.retain_moment(&row);
        self.report.plies.push(row);
        Ok(ReviewStep::Advanced)
    }

    fn retain_moment(&mut self, row: &PlyReview) {
        if row.position_before.side_to_move() != self.player || !row.label.is_mistake() {
            return;
        }
        self.report.moments.push(TeachableMoment {
            ply: row.ply,
            position_before: row.position_before.clone(),
            played: row.played,
            better: row
                .best_move
                .expect("confident label has a searched best move"),
            loss: row.loss,
            label: row.label,
            refutation: row.refutation.clone(),
        });
        self.report
            .moments
            .sort_by_key(|moment| (std::cmp::Reverse(moment.loss), moment.ply));
        self.report.moments.truncate(self.config.max_moments);
    }
}

#[cfg(test)]
mod tests;
