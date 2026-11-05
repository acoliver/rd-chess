//! Bounded, CPU-only PGN records. Indices in errors are one-based; `ply` is the
//! next move for structural errors and the failing move for move errors. RAV
//! plies use their position in the game, not their ordinal in the input file.
//!
//! Main-line comments are retained with normalized whitespace. Variations and
//! numeric/symbolic annotations are validated but not retained. Results describe
//! the recorded game (including resignation/agreement), not rules adjudication.

use super::{ChessState, FenError, INITIAL_FEN, LegalMove, MoveError};
use std::collections::BTreeMap;
use std::fmt;
use thiserror::Error;

mod export;
mod lexer;
mod parser;

pub use parser::parse;

/// The CLI also uses this limit before allocating or decoding a file.
pub const MAX_INPUT_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_VARIATION_DEPTH: usize = 32;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum PgnResult {
    WhiteWin,
    BlackWin,
    Draw,
    #[default]
    Unfinished,
}

impl PgnResult {
    fn parse(text: &str) -> Option<Self> {
        match text {
            "1-0" => Some(Self::WhiteWin),
            "0-1" => Some(Self::BlackWin),
            "1/2-1/2" => Some(Self::Draw),
            "*" => Some(Self::Unfinished),
            _ => None,
        }
    }
}

impl fmt::Display for PgnResult {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::WhiteWin => "1-0",
            Self::BlackWin => "0-1",
            Self::Draw => "1/2-1/2",
            Self::Unfinished => "*",
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum PgnErrorKind {
    #[error("input exceeds the PGN byte limit")]
    InputLimit,
    #[error("invalid PGN syntax")]
    Syntax,
    #[error("invalid or inconsistent tag pair")]
    Tag,
    #[error("invalid starting position: {0}")]
    Fen(FenError),
    #[error("invalid move: {0}")]
    Move(MoveError),
    #[error("variation nesting exceeds the PGN limit")]
    NestingLimit,
    #[error("movetext result differs from Result tag")]
    ResultMismatch,
    #[error("game has no termination marker")]
    MissingResult,
    #[error("invalid comment or comment ply")]
    Comment,
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
#[error("PGN game {game}, ply {ply}, token {token:?}: {kind}")]
pub struct PgnError {
    pub game: usize,
    pub ply: usize,
    pub token: String,
    pub kind: PgnErrorKind,
}

impl PgnError {
    fn new(game: usize, ply: usize, token: &str, kind: PgnErrorKind) -> Self {
        Self {
            game,
            ply,
            token: token.into(),
            kind,
        }
    }
}

/// A record starts at the supplied position, discarding any earlier history.
/// `tags` holds optional metadata; export supplies the roster and owns `SetUp`,
/// FEN and Result. Set player tags to names or `Computer (Level N)` as needed.
#[derive(Clone, Debug)]
pub struct PgnGame {
    pub tags: BTreeMap<String, String>,
    pub result: PgnResult,
    initial_fen: String,
    state: ChessState,
    moves: Vec<LegalMove>,
    comments: BTreeMap<usize, String>,
}

impl PgnGame {
    pub fn new(initial: &ChessState) -> Self {
        let initial_fen = initial.to_fen();
        Self {
            state: ChessState::from_fen(&initial_fen).expect("validated position"),
            initial_fen,
            tags: BTreeMap::new(),
            result: PgnResult::Unfinished,
            moves: Vec::new(),
            comments: BTreeMap::new(),
        }
    }

    pub fn initial_fen(&self) -> &str {
        &self.initial_fen
    }

    pub fn state(&self) -> &ChessState {
        &self.state
    }

    pub fn moves(&self) -> &[LegalMove] {
        &self.moves
    }

    pub fn play(&mut self, chess_move: LegalMove) -> Result<(), PgnError> {
        self.state.play(chess_move).map_err(|error| {
            PgnError::new(
                1,
                self.moves.len() + 1,
                &chess_move.to_uci(),
                PgnErrorKind::Move(error),
            )
        })?;
        self.moves.push(chess_move);
        Ok(())
    }

    /// Ply zero is a preamble; other comments follow that committed move.
    /// Control characters other than whitespace cannot be represented in PGN.
    pub fn set_comment(&mut self, ply: usize, text: &str) -> Result<(), PgnError> {
        if ply > self.moves.len() || !valid_comment(text) {
            return Err(PgnError::new(1, ply, text, PgnErrorKind::Comment));
        }
        self.comments.insert(ply, normalize_comment(text));
        Ok(())
    }

    pub fn comment(&self, ply: usize) -> Option<&str> {
        self.comments.get(&ply).map(String::as_str)
    }
}

fn normalize_comment(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn valid_comment(text: &str) -> bool {
    text.chars().all(|c| !c.is_control() || c.is_whitespace())
}

fn valid_tag_name(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|c| c.is_ascii_alphanumeric() || c == b'_')
}
