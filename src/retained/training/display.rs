use super::super::{Screen, ui::StudyState};
use super::{BoardDriver, Session, Setup, Square, Step, TaskStatus};

impl BoardDriver {
    pub(in crate::retained) fn study_ui_state(&self) -> Option<StudyState> {
        let training = self.training.as_ref()?;
        let lessons = training
            .course
            .lessons()
            .iter()
            .map(|lesson| {
                let enabled = lesson
                    .requires
                    .iter()
                    .all(|id| training.progress.completed_lessons.contains(id));
                let mark = if training.progress.completed_lessons.contains(&lesson.id) {
                    "[done]"
                } else if enabled {
                    "[open]"
                } else {
                    "[locked]"
                };
                (format!("{mark} {}", lesson.title), enabled)
            })
            .collect();
        let bands = [
            None,
            Some("under-1200"),
            Some("1200-1799"),
            Some("1800-plus"),
        ]
        .map(|band| {
            let (attempts, solved) = training
                .progress
                .bands
                .iter()
                .filter(|(key, _)| band.is_none_or(|band| key.as_str() == band))
                .fold((0, 0), |(a, s), (_, value)| {
                    (a + value.attempts, s + value.solved)
                });
            format!(
                "{}: {solved} solved / {attempts} moves",
                band.unwrap_or("Next puzzle (all bands)")
            )
        });
        Some(StudyState {
            lessons,
            bands,
            panel: training.panel.clone(),
            controls: controls(training),
            why: training.reply.is_some()
                && matches!(
                    self.screen(),
                    Screen::Play | Screen::InGameMenu | Screen::GameOver
                ),
        })
    }
    pub(in crate::retained) fn refresh_training_panel(&mut self) {
        let training = self.training.as_mut().expect("training");
        training.highlights.clear();
        let heading = match training.session.as_ref().expect("session") {
            Session::Lesson { runner, index } => {
                let lesson = &training.course.lessons()[*index];
                let (text, annotations) = match runner.step().expect("active step") {
                    Step::Explain { text, annotations } => (text, annotations),
                    Step::Task(task) => (&task.text, &task.annotations),
                };
                training
                    .highlights
                    .extend(annotations.squares.iter().map(|name| square(name)));
                let arrows = annotations
                    .arrows
                    .iter()
                    .map(|arrow| format!("{} to {}", arrow.from, arrow.to))
                    .collect::<Vec<_>>()
                    .join(", ");
                format!(
                    "{} ({}/{})\n{text}{}",
                    lesson.title,
                    runner.step_index() + 1,
                    lesson.steps.len(),
                    if arrows.is_empty() {
                        String::new()
                    } else {
                        format!("\nArrows: {arrows}")
                    }
                )
            }
            Session::Puzzle { puzzle, setup, .. } => {
                if matches!(setup, Setup::Ready) && !training.hint.is_empty() {
                    training.highlights.insert(square(
                        &puzzle.moves.split_whitespace().nth(1).expect("solver move")[..2],
                    ));
                }
                format!(
                    "Puzzle {} | {} | {}\n{}",
                    puzzle.id,
                    puzzle.rating,
                    crate::teaching::rating_band(puzzle.rating),
                    if matches!(setup, Setup::Ready) {
                        "Find the tactical continuation."
                    } else {
                        "Watch the opponent's setup move."
                    }
                )
            }
        };
        training.panel = [
            heading.as_str(),
            training.feedback.as_str(),
            training.hint.as_str(),
        ]
        .into_iter()
        .filter(|text| !text.is_empty())
        .collect::<Vec<_>>()
        .join("\n");
    }
    pub(in crate::retained) fn clear_explanation(&mut self) {
        if let Some(training) = &mut self.training {
            training.reply = None;
            if training.session.is_none() {
                training.panel.clear();
            }
        }
    }
    pub(in crate::retained) fn explain_computer_reply(&mut self) -> Result<(), String> {
        let training = self.training.as_mut().expect("training");
        if !training.panel.is_empty() && training.session.is_none() {
            training.panel.clear();
            return Ok(());
        }
        let (state, mv) = training
            .reply
            .as_ref()
            .ok_or("no computer reply to explain")?;
        let explanation =
            crate::teaching::explain_reply(state, *mv, crate::teaching::Budget::default())
                .map_err(|e| e.to_string())?;
        training.panel = explanation.sentences.join("\n");
        Ok(())
    }
}
fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').expect("validated annotation")
}

fn controls(training: &super::Training) -> [bool; 5] {
    match &training.session {
        Some(Session::Lesson { runner, .. }) => [
            runner.step_index() > 0,
            runner
                .task()
                .is_none_or(|t| t.status() == &TaskStatus::Completed),
            runner
                .task()
                .is_some_and(|t| t.status() != &TaskStatus::Completed),
            true,
            true,
        ],
        Some(Session::Puzzle { setup, runner, .. }) => [
            false,
            true,
            matches!(setup, Setup::Ready) && runner.status() != &TaskStatus::Completed,
            matches!(setup, Setup::Ready),
            true,
        ],
        None => [false; 5],
    }
}
