use super::{Annotations, Goal, MoveNode, Progress, Result, Task, TaskRunner, TaskStatus};
use super::{board, play, require, validation::identifier};
use crate::rules::ChessState;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const PUZZLE_SOURCE: &str = "https://database.lichess.org/lichess_db_puzzle.csv.zst";

pub fn rating_band(rating: u32) -> &'static str {
    match rating {
        0..1200 => "under-1200",
        1200..1800 => "1200-1799",
        _ => "1800-plus",
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PuzzlePack {
    version: u32,
    source: String,
    puzzles: Vec<Puzzle>,
}

impl PuzzlePack {
    pub fn from_json(data: &str) -> Result<Self> {
        require(data.len() <= 200_000, "puzzle pack exceeds 200 KB")?;
        let pack: Self = serde_json::from_str(data)?;
        require(pack.version == 1, "unsupported puzzle version")?;
        require(pack.source == PUZZLE_SOURCE, "unexpected puzzle source URL")?;
        require(
            !pack.puzzles.is_empty() && pack.puzzles.len() <= 500,
            "puzzle count",
        )?;
        let mut ids = BTreeSet::new();
        for puzzle in &pack.puzzles {
            require(ids.insert(&puzzle.id), "duplicate puzzle id")?;
            puzzle.task().map_err(|error| {
                super::TeachingError::Invalid(format!("puzzle {}: {error}", puzzle.id))
            })?;
        }
        Ok(pack)
    }

    pub fn bundled() -> Result<Self> {
        Self::from_json(include_str!("../../../assets/puzzles/first-pack.json"))
    }

    pub fn puzzles(&self) -> &[Puzzle] {
        &self.puzzles
    }

    pub fn puzzle(&self, id: &str) -> Option<&Puzzle> {
        self.puzzles.iter().find(|puzzle| puzzle.id == id)
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Puzzle {
    #[serde(rename = "PuzzleId")]
    pub id: String,
    #[serde(rename = "FEN")]
    pub fen: String,
    #[serde(rename = "Moves")]
    pub moves: String,
    #[serde(rename = "Rating")]
    pub rating: u32,
    #[serde(rename = "Themes")]
    pub themes: String,
    #[serde(default)]
    pub annotations: Annotations,
}

impl Puzzle {
    pub fn after_setup(&self) -> Result<ChessState> {
        let mut state = board(&self.fen)?;
        let setup = self
            .moves
            .split_whitespace()
            .next()
            .ok_or_else(|| super::TeachingError::Invalid("missing puzzle setup move".into()))?;
        play(&mut state, setup)?;
        Ok(state)
    }

    fn task(&self) -> Result<Task> {
        identifier(&self.id)?;
        require(self.rating <= 5000, "puzzle rating exceeds 5000")?;
        super::validation::text(&self.themes, 500)?;
        let moves: Vec<_> = self.moves.split_whitespace().collect();
        require(
            (2..=32).contains(&moves.len()) && moves.len().is_multiple_of(2),
            "puzzle needs setup and a final solver move",
        )?;
        let state = self.after_setup()?;
        let mut nodes = Vec::new();
        for uci in moves[1..].iter().rev() {
            nodes = vec![MoveNode {
                uci: (*uci).into(),
                next: nodes,
            }];
        }
        let goal = self.goal(moves.len())?;
        let task = Task {
            text: "Find the tactical continuation.".into(),
            fen: state.to_fen(),
            side: state.side_to_move(),
            goal,
            solutions: nodes,
            hints: vec!["Look for checks, captures and threats.".into()],
            feedback: BTreeMap::new(),
            annotations: self.annotations.clone(),
        };
        task.validate()?;
        Ok(task)
    }

    fn goal(&self, length: usize) -> Result<Goal> {
        let mut mate = false;
        for theme in self.themes.split_whitespace() {
            if theme == "mate" {
                mate = true;
            }
            if let Some(number) = theme.strip_prefix("mateIn") {
                let count = number
                    .parse::<usize>()
                    .map_err(|_| super::TeachingError::Invalid("invalid mate theme".into()))?;
                require(
                    (1..=16).contains(&count) && count * 2 == length,
                    "mate theme disagrees with solution length",
                )?;
                mate = true;
            }
        }
        Ok(if mate { Goal::AnyMate } else { Goal::Line })
    }
}

pub struct PuzzleRunner {
    puzzle: Puzzle,
    task: TaskRunner,
    progress: Progress,
}

impl PuzzleRunner {
    pub fn new(puzzle: Puzzle) -> Result<Self> {
        Self::with_progress(puzzle, Progress::default())
    }

    pub fn with_progress(puzzle: Puzzle, progress: Progress) -> Result<Self> {
        let task = TaskRunner::with_board(puzzle.task()?, puzzle.after_setup()?)?;
        Ok(Self {
            puzzle,
            task,
            progress,
        })
    }

    pub fn board(&self) -> &ChessState {
        self.task.board()
    }

    pub fn status(&self) -> &TaskStatus {
        self.task.status()
    }

    pub fn hint_move(&self) -> Option<crate::rules::LegalMove> {
        self.task.hint_move()
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn submit(&mut self, uci: &str) -> Result<()> {
        self.task.submit(uci)?;
        let band = self
            .progress
            .bands
            .entry(rating_band(self.puzzle.rating).into())
            .or_default();
        band.attempts += 1;
        if self.task.status() == &TaskStatus::Completed {
            band.solved += 1;
        }
        Ok(())
    }

    pub fn retry(&mut self) -> Result<()> {
        self.task = TaskRunner::with_board(self.puzzle.task()?, self.puzzle.after_setup()?)?;
        Ok(())
    }
}
