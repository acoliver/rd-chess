//! File-authored offline course and puzzles using the game's legal rules core.
//! This module has no viewer, search worker, renderer or persistence service.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]

use crate::{Side, rules::ChessState};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

mod progress;
mod puzzles;
mod runner;
mod validation;

pub use progress::{BandProgress, LessonProgress, Progress};
pub use puzzles::{PUZZLE_SOURCE, Puzzle, PuzzlePack, PuzzleRunner, rating_band};
pub use runner::{LessonRunner, TaskRunner, TaskStatus};

#[derive(Debug, Error)]
pub enum TeachingError {
    #[error("teaching JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

type Result<T> = std::result::Result<T, TeachingError>;

fn require(condition: bool, message: impl Into<String>) -> Result<()> {
    if condition {
        Ok(())
    } else {
        Err(TeachingError::Invalid(message.into()))
    }
}

fn board(fen: &str) -> Result<ChessState> {
    ChessState::from_fen(fen).map_err(|error| TeachingError::Invalid(format!("FEN: {error}")))
}

fn play(state: &mut ChessState, uci: &str) -> Result<()> {
    let chess_move = state
        .parse_uci(uci)
        .map_err(|error| TeachingError::Invalid(format!("move {uci}: {error}")))?;
    state
        .play(chess_move)
        .map_err(|error| TeachingError::Invalid(format!("move {uci}: {error}")))
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Course {
    version: u32,
    lessons: Vec<Lesson>,
}

impl Course {
    pub fn from_json(data: &str) -> Result<Self> {
        require(data.len() <= 262_144, "course exceeds 256 KiB")?;
        let course: Self = serde_json::from_str(data)?;
        course.validate()?;
        Ok(course)
    }

    pub fn bundled() -> Result<Self> {
        Self::from_json(include_str!("../../assets/lessons/first-course.json"))
    }

    pub fn lessons(&self) -> &[Lesson] {
        &self.lessons
    }

    pub fn lesson(&self, id: &str) -> Option<&Lesson> {
        self.lessons.iter().find(|lesson| lesson.id == id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Lesson {
    pub id: String,
    pub title: String,
    pub requires: Vec<String>,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Step {
    Explain {
        text: String,
        #[serde(default)]
        annotations: Annotations,
    },
    Task(Task),
}

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Annotations {
    pub squares: Vec<String>,
    pub arrows: Vec<Arrow>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Arrow {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Goal {
    /// Finish the explicitly requested line, not a claim of optimal play.
    Line,
    Checkmate,
    /// Accept any legal mate, including one outside the authored tree.
    AnyMate,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Task {
    pub text: String,
    pub fen: String,
    pub side: Side,
    pub goal: Goal,
    pub solutions: Vec<MoveNode>,
    pub hints: Vec<String>,
    pub feedback: BTreeMap<String, String>,
    #[serde(default)]
    pub annotations: Annotations,
}

/// Solver alternatives alternate with a single scripted opponent reply.
/// An empty `next` ends the task after the solver's move.
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct MoveNode {
    pub uci: String,
    pub next: Vec<Self>,
}
