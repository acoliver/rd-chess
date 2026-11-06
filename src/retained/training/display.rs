#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::super::{Screen, ui::StudyState};
use super::{BoardDriver, Session, Setup, Step, TaskStatus};

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
            emphasis: training.emphasis.clone(),
            controls: controls(training),
            why: training.reply.is_some()
                && matches!(
                    self.screen(),
                    Screen::Play | Screen::InGameMenu | Screen::GameOver
                ),
        })
    }
    fn study_emphasis(&self) -> Vec<String> {
        let training = self.training.as_ref().expect("training");
        let positions = self
            .board()
            .map(super::super::RetainedBoard::game)
            .into_iter()
            .chain(training.reply.as_ref().map(|(state, _)| state));
        let mut moves = training.feedback_moves.clone();
        for position in positions {
            moves.extend(
                position
                    .legal_moves()
                    .into_iter()
                    .map(|mv| position.to_san(mv).expect("legal teaching move")),
            );
        }
        moves.sort();
        moves.dedup();
        moves
    }
    pub(in crate::retained) fn refresh_training_panel(&mut self) {
        let training = self.training.as_mut().expect("training");
        training.refresh_annotations();
        let heading = match training.session.as_ref().expect("session") {
            Session::Lesson { runner, index } => {
                let lesson = &training.course.lessons()[*index];
                let text = match runner.step().expect("active step") {
                    Step::Explain { text, .. } => text,
                    Step::Task(task) => &task.text,
                };
                format!(
                    "{} ({}/{})\n{text}",
                    lesson.title,
                    runner.step_index() + 1,
                    lesson.steps.len()
                )
            }
            Session::Puzzle { puzzle, setup, .. } => {
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
        let emphasis = self.study_emphasis();
        self.training.as_mut().expect("training").emphasis = emphasis;
    }
    pub(in crate::retained) fn clear_explanation(&mut self) {
        if let Some(training) = &mut self.training {
            training.annotations.clear();
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
            training.annotations.clear();
            return Ok(());
        }
        let (state, mv) = training
            .reply
            .as_ref()
            .ok_or("no computer reply to explain")?;
        let explanation =
            crate::teaching::explain_reply(state, *mv, crate::teaching::Budget::default())
                .map_err(|e| e.to_string())?;
        let san = state.to_san(*mv).map_err(|e| e.to_string())?;
        training.panel = format!("Why {san}?\n{}", explanation.sentences.join("\n"));
        training.emphasis = state
            .legal_moves()
            .into_iter()
            .map(|mv| state.to_san(mv).expect("legal reply alternative"))
            .collect();
        let annotations = self.explanation_annotations(
            explanation
                .alternative_line
                .as_ref()
                .and_then(|line| line.first().copied()),
            explanation.facts.iter().find_map(|fact| {
                matches!(
                    fact.kind,
                    crate::teaching::FactKind::HangingPiece { .. }
                        | crate::teaching::FactKind::Exchange {
                            gain: i32::MIN..=-1
                        }
                )
                .then(|| fact.line.get(1).copied())
                .flatten()
            }),
        );
        self.training.as_mut().expect("training").annotations = annotations;
        Ok(())
    }
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
