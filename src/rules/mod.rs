//! Legal chess state, independent of the arbitrary scene/editor [`Position`].
//!
//! A 0x88 mailbox makes off-board rays explicit. Pin and check masks filter
//! ordinary moves; king moves and en passant use copy-make king-safety checks.
//! Committed moves retain changed squares and incremental Zobrist history.
//! Ivory is white and Onyx is black. Moves are always revalidated at application.
//!
//! FEN imports validate local legality and material budgets, not full retrograde
//! reachability. FEN cannot recover repetition history: persist an initial FEN
//! and moves instead. SAN parsing accepts exact standard SAN, without annotations
//! or alternate spellings. This module implements standard chess, not Chess960.
//!
//! Automatic dead-material recognition covers bare kings, one minor against a
//! bare king, and bishops confined to one square colour with no other non-kings.
//! Other dead positions, including blocked pawn fortresses, are not adjudicated.
//! Clocks are move counters only; timed play and flag-fall are outside this core.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]

use crate::{Piece, PieceKind, Position, SAVE_VERSION, Side, Square};
use board::{Core, Man, index, opposite, square};
use thiserror::Error;

mod attacks;
mod board;
mod fen;
mod hash;
mod movegen;
mod notation;
#[cfg(test)]
mod optimization_tests;
mod outcome;
mod safety;
mod transition;
mod validation;

pub use fen::FenError;
pub use outcome::{DrawClaims, DrawReason, GameStatus};

pub const INITIAL_FEN: &str = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";

/// Castling entitlements, independent of whether a castle is currently legal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CastlingRights(u8);

impl CastlingRights {
    pub fn king_side(self, side: Side) -> bool {
        self.0 & Self::bit(side, true) != 0
    }

    pub fn queen_side(self, side: Side) -> bool {
        self.0 & Self::bit(side, false) != 0
    }

    const fn bit(side: Side, king_side: bool) -> u8 {
        let shift = match side {
            Side::Ivory => 0,
            Side::Onyx => 2,
        };
        (if king_side { 1 } else { 2 }) << shift
    }

    fn remove_side(&mut self, side: Side) {
        self.0 &= !(Self::bit(side, true) | Self::bit(side, false));
    }

    fn remove_corner(&mut self, location: u8) {
        self.0 &= !match location {
            0 => 2,
            7 => 1,
            112 => 8,
            119 => 4,
            _ => 0,
        };
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PromotionPiece {
    Queen,
    Rook,
    Bishop,
    Knight,
}

impl PromotionPiece {
    const ALL: [Self; 4] = [Self::Queen, Self::Rook, Self::Bishop, Self::Knight];

    const fn kind(self) -> PieceKind {
        match self {
            Self::Queen => PieceKind::Queen,
            Self::Rook => PieceKind::Rook,
            Self::Bishop => PieceKind::Bishop,
            Self::Knight => PieceKind::Knight,
        }
    }
}

/// Obtained only from a legal position. Applicability is checked again by `play`.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct LegalMove {
    from: u8,
    to: u8,
    promotion: Option<PromotionPiece>,
}

impl LegalMove {
    pub fn from(self) -> Square {
        square(self.from)
    }
    pub fn to(self) -> Square {
        square(self.to)
    }
    pub fn promotion(self) -> Option<PromotionPiece> {
        self.promotion
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Snapshot {
    core: transition::BoardUndo,
    halfmove: u32,
    fullmove: u32,
    chess_move: LegalMove,
}

/// A validated playable state. Editor documents are output only, never authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChessState {
    core: Core,
    halfmove: u32,
    fullmove: u32,
    repetition: Vec<u64>,
    undo: Vec<Snapshot>,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum MoveError {
    #[error("move is not legal in this position")]
    IllegalMove,
    #[error("invalid or nonstandard move notation")]
    Notation,
    #[error("no committed move to undo")]
    NoHistory,
    #[error("move counter exceeds the supported u32 range")]
    CounterOverflow,
}

impl ChessState {
    pub fn initial() -> Self {
        Self::from_fen(INITIAL_FEN).expect("standard initial position is legal")
    }

    pub fn side_to_move(&self) -> Side {
        self.core.side
    }
    pub fn castling_rights(&self) -> CastlingRights {
        self.core.rights
    }
    pub fn en_passant_square(&self) -> Option<Square> {
        self.core.ep.map(square)
    }
    pub fn halfmove_clock(&self) -> u32 {
        self.halfmove
    }
    pub fn fullmove_number(&self) -> u32 {
        self.fullmove
    }
    pub fn repetition_history(&self) -> &[u64] {
        &self.repetition
    }
    pub fn position_hash(&self) -> u64 {
        *self.repetition.last().expect("initial hash exists")
    }
    pub fn in_check(&self) -> bool {
        self.core.in_check(self.core.side)
    }

    /// Invalid editor square coordinates return `None`, without indexing the board.
    pub fn piece_at(&self, location: Square) -> Option<(Side, PieceKind)> {
        index(location)
            .and_then(|location| self.core.at(location))
            .map(|man| (man.side, man.kind))
    }

    /// Generates king-safe moves, regardless of whether a draw has been claimed.
    /// Game-level resignation, agreement and claimed draws belong to the caller.
    pub fn legal_moves(&self) -> Vec<LegalMove> {
        self.core.legal_moves()
    }

    /// Initial position and committed moves, including reversible repetition history.
    pub fn record(&self) -> (String, Vec<String>) {
        let mut initial = self.clone();
        for _ in 0..self.undo.len() {
            initial.undo().expect("committed move has undo history");
        }
        (
            initial.to_fen(),
            self.undo.iter().map(|s| s.chess_move.to_uci()).collect(),
        )
    }

    /// Apply a legal move transactionally, retaining exact state for `undo`.
    /// Automatic outcomes are reported by `status`; callers end play on them.
    pub fn play(&mut self, chess_move: LegalMove) -> Result<(), MoveError> {
        if !self.legal_moves().contains(&chess_move) {
            return Err(MoveError::IllegalMove);
        }
        self.play_generated(chess_move)
    }

    /// Only for moves freshly generated from this exact state. Public play
    /// continues to revalidate stale/caller-supplied moves transactionally.
    pub(crate) fn play_generated(&mut self, chess_move: LegalMove) -> Result<(), MoveError> {
        debug_assert!(self.legal_moves().contains(&chess_move));
        let man = self.core.at(chess_move.from).expect("legal mover exists");
        let halfmove = if man.kind == PieceKind::Pawn || self.core.is_capture(chess_move) {
            0
        } else {
            self.halfmove
                .checked_add(1)
                .ok_or(MoveError::CounterOverflow)?
        };
        let fullmove = self
            .fullmove
            .checked_add(u32::from(self.core.side == Side::Onyx))
            .ok_or(MoveError::CounterOverflow)?;
        let saved = self.core.remember(chess_move);
        self.undo.push(Snapshot {
            core: saved,
            halfmove: self.halfmove,
            fullmove: self.fullmove,
            chess_move,
        });
        self.core.make(chess_move);
        self.halfmove = halfmove;
        self.fullmove = fullmove;
        self.repetition
            .push(self.core.hash_after(self.position_hash(), &saved));
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), MoveError> {
        let saved = self.undo.pop().ok_or(MoveError::NoHistory)?;
        self.core.restore(saved.core);
        self.halfmove = saved.halfmove;
        self.fullmove = saved.fullmove;
        self.repetition.pop();
        Ok(())
    }

    /// Scene IDs survive moves, captures/undo and promotion within this state.
    /// A fresh FEN import assigns fresh identities from its occupied squares.
    pub fn to_position(&self, seed: u64) -> Position {
        let pieces = self
            .core
            .men()
            .map(|(location, man)| Piece {
                id: format!("rules-{}", man.id),
                side: man.side,
                kind: man.kind,
                square: square(location),
            })
            .collect();
        Position {
            format: "chess3d-position".into(),
            version: SAVE_VERSION,
            seed,
            style: "nocturne-ivory".into(),
            pieces,
        }
    }
}

#[cfg(test)]
mod edge_tests;
#[cfg(test)]
mod notation_tests;
#[cfg(test)]
mod outcome_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod performance;
