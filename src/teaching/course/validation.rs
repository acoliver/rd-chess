use super::{Annotations, Course, Goal, Lesson, MoveNode, Result, Step, Task, TeachingError};
use super::{board, play, require};
use crate::rules::{ChessState, GameStatus};
use std::collections::BTreeSet;

pub(super) fn text(value: &str, limit: usize) -> Result<()> {
    require(
        !value.trim().is_empty()
            && value.len() <= limit
            && value
                .bytes()
                .all(|b| b.is_ascii_graphic() || b" \n".contains(&b)),
        format!("text must be nonempty printable English ASCII, at most {limit} bytes"),
    )
}

pub(super) fn identifier(value: &str) -> Result<()> {
    require(
        !value.is_empty()
            && value.len() <= 64
            && value
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b)),
        "invalid id",
    )
}

fn square(value: &str) -> Result<()> {
    let bytes = value.as_bytes();
    require(
        bytes.len() == 2 && (b'a'..=b'h').contains(&bytes[0]) && (b'1'..=b'8').contains(&bytes[1]),
        "invalid annotation square",
    )
}

impl Annotations {
    fn validate(&self) -> Result<()> {
        require(
            self.squares.len() <= 64 && self.arrows.len() <= 32,
            "too many annotations",
        )?;
        for name in &self.squares {
            square(name)?;
        }
        for arrow in &self.arrows {
            square(&arrow.from)?;
            square(&arrow.to)?;
            require(arrow.from != arrow.to, "annotation arrow has no length")?;
        }
        Ok(())
    }
}

impl Course {
    pub(super) fn validate(&self) -> Result<()> {
        require(self.version == 1, "unsupported course version")?;
        require(
            !self.lessons.is_empty() && self.lessons.len() <= 128,
            "course lesson count",
        )?;
        let mut ids = BTreeSet::new();
        for lesson in &self.lessons {
            require(
                ids.insert(&lesson.id),
                format!("duplicate lesson id {}", lesson.id),
            )?;
            lesson.validate().map_err(|error| {
                TeachingError::Invalid(format!("lesson {}: {error}", lesson.id))
            })?;
        }
        let mut complete = BTreeSet::new();
        for lesson in &self.lessons {
            self.visit(lesson, &mut BTreeSet::new(), &mut complete)?;
        }
        Ok(())
    }

    fn visit<'a>(
        &'a self,
        lesson: &'a Lesson,
        visiting: &mut BTreeSet<&'a str>,
        complete: &mut BTreeSet<&'a str>,
    ) -> Result<()> {
        if complete.contains(lesson.id.as_str()) {
            return Ok(());
        }
        require(visiting.insert(&lesson.id), "prerequisite cycle")?;
        for id in &lesson.requires {
            let prerequisite = self
                .lesson(id)
                .ok_or_else(|| TeachingError::Invalid(format!("missing prerequisite {id}")))?;
            self.visit(prerequisite, visiting, complete)?;
        }
        visiting.remove(lesson.id.as_str());
        complete.insert(&lesson.id);
        Ok(())
    }
}

impl Lesson {
    pub(super) fn validate(&self) -> Result<()> {
        identifier(&self.id)?;
        text(&self.title, 120)?;
        require(
            !self.steps.is_empty() && self.steps.len() <= 64,
            "lesson step count",
        )?;
        let mut motifs = BTreeSet::new();
        for motif in &self.motifs {
            require(motifs.insert(motif), "duplicate motif")?;
        }
        let mut requires = BTreeSet::new();
        for prerequisite in &self.requires {
            identifier(prerequisite)?;
            require(requires.insert(prerequisite), "duplicate prerequisite")?;
        }
        for (index, step) in self.steps.iter().enumerate() {
            step.validate()
                .map_err(|error| TeachingError::Invalid(format!("step {index}: {error}")))?;
        }
        Ok(())
    }
}

impl Step {
    fn validate(&self) -> Result<()> {
        match self {
            Self::Explain {
                text: paragraph,
                annotations,
            } => {
                text(paragraph, 1500)?;
                annotations.validate()
            }
            Self::Task(task) => task.validate(),
        }
    }
}

impl Task {
    pub fn validate(&self) -> Result<()> {
        text(&self.text, 1000)?;
        require(
            !self.hints.is_empty() && self.hints.len() <= 8,
            "task hint count",
        )?;
        for hint in &self.hints {
            text(hint, 500)?;
        }
        require(self.feedback.len() <= 64, "too much feedback")?;
        for (uci, feedback) in &self.feedback {
            validate_uci(uci)?;
            text(feedback, 500)?;
        }
        self.annotations.validate()?;
        let state = board(&self.fen)?;
        require(
            state.side_to_move() == self.side,
            "task side disagrees with FEN",
        )?;
        self.validate_nodes(&state, &self.solutions, 0, &mut 0)
    }

    fn validate_nodes(
        &self,
        state: &ChessState,
        nodes: &[MoveNode],
        depth: usize,
        count: &mut usize,
    ) -> Result<()> {
        require(depth < 32 && *count < 2048, "solution tree limit")?;
        require(!nodes.is_empty(), "unreachable empty solution")?;
        require(
            depth.is_multiple_of(2) || nodes.len() == 1,
            "unreachable opponent branch: script one reply",
        )?;
        let mut unique = BTreeSet::new();
        for node in nodes {
            *count += 1;
            require(*count <= 2048, "solution tree limit")?;
            require(unique.insert(&node.uci), "unreachable duplicate branch")?;
            require(
                matches!(state.status(), GameStatus::Ongoing { .. }),
                "unreachable branch after terminal position",
            )?;
            let mut next = state.clone();
            play(&mut next, &node.uci)?;
            if node.next.is_empty() {
                require(
                    depth.is_multiple_of(2),
                    "unreachable solution ending on opponent turn",
                )?;
                require(
                    self.goal == Goal::Line || self.is_mate(&next),
                    "task mate goal does not end in mate",
                )?;
            } else {
                self.validate_nodes(&next, &node.next, depth + 1, count)?;
            }
        }
        Ok(())
    }

    pub(super) fn is_mate(&self, state: &ChessState) -> bool {
        matches!(state.status(), GameStatus::Checkmate { winner } if winner == self.side)
    }
}

fn validate_uci(value: &str) -> Result<()> {
    require(value.is_ascii(), "feedback move must be ASCII UCI")?;
    require(
        value.len() == 4 || value.len() == 5,
        "feedback move must be UCI",
    )?;
    square(&value[..2])?;
    square(&value[2..4])?;
    require(
        value.len() == 4 || b"qrbn".contains(&value.as_bytes()[4]),
        "invalid promotion",
    )
}
