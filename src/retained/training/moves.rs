#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::{BoardDriver, ChessState, LegalMove, Session, TaskStatus};

impl BoardDriver {
    pub(in crate::retained) fn submit_training_move(
        &mut self,
        mv: LegalMove,
    ) -> Result<(), String> {
        let before = self.board.as_ref().expect("board").game().clone();
        let training = self.training.as_mut().expect("training");
        training.feedback_moves = before
            .legal_moves()
            .into_iter()
            .map(|mv| before.to_san(mv).expect("legal teaching move"))
            .collect();
        let (status, game, puzzle) = match training.session.as_mut().expect("session") {
            Session::Lesson { runner, .. } => {
                runner.submit(&mv.to_uci()).map_err(|e| e.to_string())?;
                let task = runner.task().expect("board task");
                (task.status().clone(), task.board().clone(), false)
            }
            Session::Puzzle { runner, .. } => {
                runner.submit(&mv.to_uci()).map_err(|e| e.to_string())?;
                (runner.status().clone(), runner.board().clone(), true)
            }
        };
        training.feedback = match &status {
            TaskStatus::Wrong(text) => format!(
                "{text}{}",
                if puzzle {
                    refutation(&before, mv)?
                } else {
                    String::new()
                }
            ),
            TaskStatus::Completed => "Completed. Choose Next to continue.".into(),
            TaskStatus::Correct => "Correct. The opponent replied; continue the line.".into(),
            TaskStatus::AwaitingMove => unreachable!("submitted move"),
        };
        training.save_progress();
        // The runner commits accepted solver and opponent plies together. A wrong
        // answer never reaches retained geometry, so the checkpoint stays visible.
        if matches!(status, TaskStatus::Wrong(_)) {
            self.clear_selection()?;
        } else {
            let edits = self.board.as_mut().expect("board").synchronize_game(game);
            self.enqueue_move(edits)?;
        }
        self.persist_training_progress();
        self.refresh_training_panel();
        self.sync_menu_ui()
    }
}
fn refutation(state: &ChessState, mv: LegalMove) -> Result<String, String> {
    let explanation = crate::teaching::explain_move(state, mv, crate::teaching::Budget::default())
        .map_err(|e| e.to_string())?;
    if explanation.facts.is_empty() {
        return Ok(String::new());
    }
    Ok(format!("\n{}", explanation.sentences.join(" ")))
}
