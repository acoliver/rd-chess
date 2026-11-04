//! Window teaching controller. Runners remain the authority for accepted moves.
mod actions;
mod display;
mod moves;
use super::{BoardDriver, menus::MenuAction};
use crate::{
    Square,
    rules::{ChessState, LegalMove},
    teaching::{
        Course, LessonRunner, Progress, Puzzle, PuzzlePack, PuzzleRunner, Step, TaskStatus,
    },
};
use std::{collections::BTreeSet, time::Duration};

pub(super) struct Training {
    pub course: Course,
    pub puzzles: PuzzlePack,
    pub progress: Progress,
    session: Option<Session>,
    pub panel: String,
    feedback: String,
    hint: String,
    highlights: BTreeSet<Square>,
    puzzle_cursor: usize,
    band: Option<&'static str>,
    previous_camera: Option<worldrunner::scene::viewer::OrbitCamera>,
    pub reply: Option<(ChessState, LegalMove)>,
}

enum Session {
    Lesson {
        runner: LessonRunner,
        index: usize,
    },
    Puzzle {
        runner: PuzzleRunner,
        puzzle: Puzzle,
        setup: Setup,
    },
}
#[derive(Clone, Copy)]
enum Setup {
    Waiting(Option<Duration>),
    Ready,
}
impl Training {
    pub fn bundled() -> Result<Self, String> {
        Ok(Self {
            course: Course::bundled().map_err(|e| e.to_string())?,
            puzzles: PuzzlePack::bundled().map_err(|e| e.to_string())?,
            progress: Progress::default(),
            session: None,
            panel: String::new(),
            feedback: String::new(),
            hint: String::new(),
            highlights: BTreeSet::new(),
            puzzle_cursor: 0,
            band: None,
            reply: None,
            previous_camera: None,
        })
    }
    pub fn leave(&mut self) -> Option<worldrunner::scene::viewer::OrbitCamera> {
        self.save_progress();
        self.session = None;
        self.panel.clear();
        self.highlights.clear();
        self.reply = None;
        self.previous_camera.take()
    }
    fn save_progress(&mut self) {
        self.progress = match &self.session {
            Some(Session::Lesson { runner, .. }) => runner.progress().clone(),
            Some(Session::Puzzle { runner, .. }) => runner.progress().clone(),
            None => return,
        };
    }
    fn clear_feedback(&mut self) {
        self.feedback.clear();
        self.hint.clear();
    }
    fn accepts_input(&self) -> bool {
        match &self.session {
            Some(Session::Lesson { runner, .. }) => runner
                .task()
                .is_some_and(|task| task.status() != &TaskStatus::Completed),
            Some(Session::Puzzle {
                runner,
                setup: Setup::Ready,
                ..
            }) => runner.status() != &TaskStatus::Completed,
            _ => false,
        }
    }
}
impl BoardDriver {
    pub fn progress_error(&self) -> Option<&super::persistence::ProgressError> {
        self.storage
            .as_ref()
            .and_then(|storage| storage.progress_error.as_ref())
    }
    pub(super) fn persist_training_progress(&mut self) {
        let Some(training) = &mut self.training else {
            return;
        };
        training.save_progress();
        if let Some(storage) = &mut self.storage
            && training.progress != storage.progress
        {
            match storage.save_progress(&training.progress) {
                Ok(()) => storage.progress_error = None,
                Err(error) => storage.progress_error = Some(error),
            }
        }
    }
    pub fn progress(&self) -> Option<&Progress> {
        self.training.as_ref().map(|training| &training.progress)
    }
    pub fn panel_text(&self) -> &str {
        self.training
            .as_ref()
            .map_or("", |training| training.panel.as_str())
    }
    pub fn training_highlights(&self) -> BTreeSet<Square> {
        self.training
            .as_ref()
            .map_or_else(BTreeSet::new, |training| training.highlights.clone())
    }
    pub(super) fn training_screen(&self) -> bool {
        matches!(self.screen(), super::Screen::Lesson | super::Screen::Puzzle)
    }
    pub(super) fn training_blocked(&self) -> bool {
        self.training_screen() && !self.training.as_ref().expect("training").accepts_input()
    }
    pub(super) fn advance_training(&mut self, elapsed: Duration) -> Result<(), String> {
        if self.presenting || !self.pending.borrow().is_empty() {
            return Ok(());
        }
        let Some(Session::Puzzle { runner, setup, .. }) =
            self.training.as_mut().and_then(|t| t.session.as_mut())
        else {
            return Ok(());
        };
        match setup {
            Setup::Waiting(None) => {
                *setup = Setup::Waiting(Some(elapsed + Duration::from_millis(350)));
            }
            Setup::Waiting(Some(deadline)) if elapsed >= *deadline => {
                *setup = Setup::Ready;
                let game = runner.board().clone();
                self.replace_training_board(game)?;
                self.refresh_training_panel();
            }
            _ => {}
        }
        Ok(())
    }
    fn replace_training_board(&mut self, game: ChessState) -> Result<(), String> {
        let edits = self
            .board
            .as_mut()
            .expect("playable board")
            .set_position(game);
        self.enqueue_move(edits)
    }
}

#[cfg(test)]
#[path = "training_tests.rs"]
mod tests;

/// Engine-projected board centers for deterministic input tests, with the study camera.
pub fn training_move_pixels(uci: &str, size: [u32; 2]) -> Result<[[f64; 2]; 2], String> {
    if !uci.is_ascii() || uci.len() < 4 {
        return Err("expected UCI coordinates".into());
    }
    let view = worldrunner::scene::gpu::SceneView::new(
        actions::study_pose(),
        crate::presentation::scene_camera(),
        worldrunner::scene::gpu::Viewport {
            width: size[0],
            height: size[1],
        },
    )
    .map_err(|e| e.to_string())?;
    let mut points = [[0.; 2]; 2];
    for (index, name) in [&uci[..2], &uci[2..4]].into_iter().enumerate() {
        let bytes = name.as_bytes();
        if !(b'a'..=b'h').contains(&bytes[0]) || !(b'1'..=b'8').contains(&bytes[1]) {
            return Err("invalid square".into());
        }
        let point = view
            .resolved_view()
            .world_to_screen(glam::Vec3::new(
                f32::from(bytes[0] - b'a') - 3.5,
                f32::from(bytes[1] - b'1') - 3.5,
                0.05,
            ))
            .map_err(|e| e.to_string())?;
        points[index] = [f64::from(point.x), f64::from(point.y)];
    }
    Ok(points)
}
