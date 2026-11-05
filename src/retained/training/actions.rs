use super::super::Screen;
use super::{
    BoardDriver, ChessState, LessonRunner, MenuAction, Puzzle, PuzzleRunner, Session, Setup, Step,
};

impl BoardDriver {
    pub(in crate::retained) fn training_action(
        &mut self,
        action: MenuAction,
    ) -> Result<(), String> {
        match action {
            MenuAction::Lessons => self.transition(Screen::Lessons)?,
            MenuAction::Puzzles => self.transition(Screen::Puzzles)?,
            MenuAction::Lesson(index) => self.begin_lesson(index)?,
            MenuAction::PuzzleBand(band) => {
                self.training.as_mut().expect("training").band = band;
                self.next_puzzle()?;
            }
            MenuAction::StudyNext => self.study_next()?,
            MenuAction::StudyBack => self.study_back()?,
            MenuAction::StudyHint => self.study_hint(),
            MenuAction::StudyRetry => self.study_retry()?,
            MenuAction::StudyExit => {
                let training = self.training.as_mut().expect("training");
                training.save_progress();
                training.session = None;
                training.panel.clear();
                training.highlights.clear();
                self.transition(if self.screen() == Screen::Lesson {
                    Screen::Lessons
                } else {
                    Screen::Puzzles
                })?;
            }
            MenuAction::Why => {
                self.explain_computer_reply()?;
                self.transition(Screen::Play)?;
            }
            _ => unreachable!("teaching action"),
        }
        self.persist_training_progress();
        self.sync_menu_ui()
    }
    fn begin_lesson(&mut self, index: usize) -> Result<(), String> {
        let training = self.training.as_mut().expect("training");
        let lesson = &training.course.lessons()[index];
        if !lesson
            .requires
            .iter()
            .all(|id| training.progress.completed_lessons.contains(id))
        {
            return Ok(());
        }
        let runner = LessonRunner::with_progress(lesson.clone(), training.progress.clone())
            .map_err(|e| e.to_string())?;
        training.session = Some(Session::Lesson { runner, index });
        training.clear_feedback();
        self.start_training(Screen::Lesson)?;
        self.lesson_checkpoint()
    }
    pub(in crate::retained) fn begin_puzzle(&mut self, puzzle: Puzzle) -> Result<(), String> {
        let training = self.training.as_mut().expect("training");
        let runner = PuzzleRunner::with_progress(puzzle.clone(), training.progress.clone())
            .map_err(|e| e.to_string())?;
        let game = ChessState::from_fen(&puzzle.fen).map_err(|e| e.to_string())?;
        training.session = Some(Session::Puzzle {
            runner,
            puzzle,
            setup: Setup::Waiting(None),
        });
        training.clear_feedback();
        self.start_training(Screen::Puzzle)?;
        self.replace_training_board(game)?;
        self.refresh_training_panel();
        Ok(())
    }
    fn start_training(&mut self, screen: Screen) -> Result<(), String> {
        self.computer_changed();
        self.computer = None;
        self.menus.result = None;
        self.transition(screen)?;
        let camera = self.camera.as_mut().expect("configured camera");
        let training = self.training.as_mut().expect("training");
        if training.previous_camera.is_none() {
            training.previous_camera = Some(camera.orbit.clone());
        }
        camera.orbit = worldrunner::scene::viewer::OrbitCamera::new(
            study_pose(),
            worldrunner::scene::viewer::OrbitLimits {
                min_distance_millimeters: 2_000.,
                max_distance_millimeters: 40_000.,
                min_elevation_radians: 0.1,
                max_elevation_radians: 1.47,
            },
        )
        .map_err(|e| e.to_string())?;
        camera.dirty = true;
        Ok(())
    }
    fn lesson_checkpoint(&mut self) -> Result<(), String> {
        let Some(Session::Lesson { runner, index }) =
            self.training.as_ref().and_then(|t| t.session.as_ref())
        else {
            unreachable!("lesson");
        };
        let training = self.training.as_ref().expect("training");
        // Explanations illustrate the following task, including its authored position.
        let game = runner.task().map(|task| task.board().clone()).or_else(|| {
            training.course.lessons()[*index].steps[runner
                .step_index()
                .min(training.course.lessons()[*index].steps.len())..]
                .iter()
                .find_map(|step| {
                    if let Step::Task(task) = step {
                        Some(ChessState::from_fen(&task.fen).expect("validated task"))
                    } else {
                        None
                    }
                })
        });
        if let Some(game) = game {
            self.replace_training_board(game)?;
        }
        self.refresh_training_panel();
        Ok(())
    }
    fn study_next(&mut self) -> Result<(), String> {
        if self.screen() == Screen::Puzzle {
            return self.next_puzzle();
        }
        let training = self.training.as_mut().expect("training");
        let Some(Session::Lesson { runner, .. }) = &mut training.session else {
            unreachable!("lesson");
        };
        runner.advance().map_err(|e| e.to_string())?;
        let complete = runner.completed();
        training.save_progress();
        training.clear_feedback();
        if complete {
            training.session = None;
            training.panel.clear();
            training.highlights.clear();
            self.transition(Screen::Lessons)
        } else {
            self.lesson_checkpoint()
        }
    }
    fn study_back(&mut self) -> Result<(), String> {
        let training = self.training.as_mut().expect("training");
        let Some(Session::Lesson { runner, .. }) = &mut training.session else {
            unreachable!("lesson");
        };
        runner.back().map_err(|e| e.to_string())?;
        training.clear_feedback();
        self.lesson_checkpoint()
    }
    fn study_retry(&mut self) -> Result<(), String> {
        let training = self.training.as_mut().expect("training");
        let game = match training.session.as_mut().expect("session") {
            Session::Lesson { runner, .. } => {
                runner.retry().map_err(|e| e.to_string())?;
                runner.task().map(|task| task.board().clone())
            }
            Session::Puzzle { runner, setup, .. } => {
                runner.retry().map_err(|e| e.to_string())?;
                *setup = Setup::Ready;
                Some(runner.board().clone())
            }
        };
        training.clear_feedback();
        if let Some(game) = game {
            self.replace_training_board(game)?;
        }
        self.refresh_training_panel();
        Ok(())
    }
    fn study_hint(&mut self) {
        let training = self.training.as_mut().expect("training");
        match training.session.as_mut().expect("session") {
            Session::Lesson { runner, .. } => {
                if let Some(hint) = runner.hint() {
                    training.hint = hint.to_owned();
                }
            }
            Session::Puzzle {
                puzzle,
                setup: Setup::Ready,
                ..
            } => {
                let uci = puzzle
                    .moves
                    .split_whitespace()
                    .nth(1)
                    .expect("validated line");
                training.hint = format!("Look at the piece on {}.", &uci[..2]);
            }
            Session::Puzzle { .. } => {}
        }
        training.save_progress();
        self.refresh_training_panel();
    }
    fn next_puzzle(&mut self) -> Result<(), String> {
        let training = self.training.as_mut().expect("training");
        training.save_progress();
        let puzzles = training.puzzles.puzzles();
        let index = (0..puzzles.len())
            .map(|offset| (training.puzzle_cursor + offset) % puzzles.len())
            .find(|&index| {
                training
                    .band
                    .is_none_or(|band| crate::teaching::rating_band(puzzles[index].rating) == band)
            })
            .expect("bundled rating band has puzzles");
        let puzzle = puzzles[index].clone();
        training.puzzle_cursor = (index + 1) % puzzles.len();
        self.begin_puzzle(puzzle)
    }
}

pub(super) fn study_pose() -> worldrunner::scene::gpu::CameraPose {
    worldrunner::scene::gpu::CameraPose {
        eye_millimeters: [0, -4200, 17_000],
        target_millimeters: [0, -2200, 0],
    }
}
