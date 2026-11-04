use super::{Goal, Lesson, MoveNode, Progress, Result, Step, Task, board, play, require};
use crate::rules::ChessState;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TaskStatus {
    AwaitingMove,
    Correct,
    Wrong(String),
    Completed,
}

pub struct TaskRunner {
    task: Task,
    board: ChessState,
    choices: Vec<MoveNode>,
    status: TaskStatus,
    hint_level: usize,
}

impl TaskRunner {
    pub fn new(task: Task) -> Result<Self> {
        task.validate()?;
        Ok(Self {
            board: board(&task.fen)?,
            choices: task.solutions.clone(),
            task,
            status: TaskStatus::AwaitingMove,
            hint_level: 0,
        })
    }

    pub fn board(&self) -> &ChessState {
        &self.board
    }

    pub fn status(&self) -> &TaskStatus {
        &self.status
    }

    pub fn hint_level(&self) -> usize {
        self.hint_level
    }

    pub fn hint(&mut self) -> Option<&str> {
        if self.status == TaskStatus::Completed || self.hint_level == self.task.hints.len() {
            return None;
        }
        self.hint_level += 1;
        Some(&self.task.hints[self.hint_level - 1])
    }

    /// Wrong answers leave the board and branch checkpoint unchanged.
    /// Correct advances one solver move plus its single scripted opponent reply.
    pub fn submit(&mut self, uci: &str) -> Result<()> {
        require(
            self.status != TaskStatus::Completed,
            "task already completed",
        )?;
        let mut next = self.board.clone();
        if play(&mut next, uci).is_err() {
            self.wrong(uci, "That move is not legal here.");
            return Ok(());
        }
        if self.task.goal == Goal::AnyMate && self.task.is_mate(&next) {
            self.board = next;
            self.status = TaskStatus::Completed;
            return Ok(());
        }
        let Some(node) = self.choices.iter().find(|node| node.uci == uci) else {
            self.wrong(uci, "That move does not meet this task. Try again.");
            return Ok(());
        };
        let choices = if let Some(reply) = node.next.first() {
            play(&mut next, &reply.uci)?;
            reply.next.clone()
        } else {
            Vec::new()
        };
        self.board = next;
        self.choices = choices;
        self.status = if self.choices.is_empty() {
            TaskStatus::Completed
        } else {
            TaskStatus::Correct
        };
        Ok(())
    }

    fn wrong(&mut self, uci: &str, fallback: &str) {
        self.status = TaskStatus::Wrong(
            self.task
                .feedback
                .get(uci)
                .map_or(fallback, String::as_str)
                .into(),
        );
    }
}

pub struct LessonRunner {
    lesson: Lesson,
    index: usize,
    task: Option<TaskRunner>,
    progress: Progress,
}

impl LessonRunner {
    pub fn new(lesson: Lesson) -> Result<Self> {
        Self::with_progress(lesson, Progress::default())
    }

    pub fn with_progress(lesson: Lesson, progress: Progress) -> Result<Self> {
        lesson.validate()?;
        let mut runner = Self {
            lesson,
            index: 0,
            task: None,
            progress,
        };
        runner.start_step()?;
        Ok(runner)
    }

    pub fn step(&self) -> Option<&Step> {
        self.lesson.steps.get(self.index)
    }

    pub fn task(&self) -> Option<&TaskRunner> {
        self.task.as_ref()
    }

    pub fn completed(&self) -> bool {
        self.index == self.lesson.steps.len()
    }

    pub fn progress(&self) -> &Progress {
        &self.progress
    }

    pub fn submit(&mut self, uci: &str) -> Result<()> {
        let task = self
            .task
            .as_mut()
            .ok_or_else(|| super::TeachingError::Invalid("step is not a board task".into()))?;
        task.submit(uci)?;
        self.progress
            .lessons
            .entry(self.lesson.id.clone())
            .or_default()
            .attempts += 1;
        Ok(())
    }

    pub fn hint(&mut self) -> Option<&str> {
        let hint = self.task.as_mut()?.hint()?;
        self.progress
            .lessons
            .entry(self.lesson.id.clone())
            .or_default()
            .hints_used += 1;
        Some(hint)
    }

    /// Retry resets the board and hint cursor but retains attempt totals.
    pub fn retry(&mut self) -> Result<()> {
        require(!self.completed(), "lesson already completed")?;
        self.start_step()
    }

    pub fn advance(&mut self) -> Result<()> {
        require(!self.completed(), "lesson already completed")?;
        require(
            self.task
                .as_ref()
                .is_none_or(|task| task.status == TaskStatus::Completed),
            "finish the task before advancing",
        )?;
        self.index += 1;
        if self.completed() {
            self.progress
                .completed_lessons
                .insert(self.lesson.id.clone());
        }
        self.start_step()
    }

    fn start_step(&mut self) -> Result<()> {
        self.task = match self.step() {
            Some(Step::Task(task)) => Some(TaskRunner::new(task.clone())?),
            _ => None,
        };
        Ok(())
    }
}
