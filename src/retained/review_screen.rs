#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::{
    BoardDriver, Screen,
    menus::MenuAction,
    persistence::{GamePayload, SavedMode},
};
use crate::{
    review::{Progress, Review, ReviewConfig, ReviewReport},
    rules::{ChessState, LegalMove},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use worldrunner::scene::viewer::{RetainedHost, WorkQueue, WorkSubmitError, WorkTicket};

mod actions;
mod feedback;
mod lessons;
mod links;
#[cfg(test)]
mod tests;

pub(super) struct ReviewSession {
    pub payload: GamePayload,
    recorded: ChessState,
    camera: Option<super::driver::CameraControls>,
    pub parent: Screen,
    pub core: Option<Review>,
    pub report: ReviewReport,
    feedback: feedback::Feedback,
    pub progress: Progress,
    queue: Option<WorkQueue<Completion>>,
    pending: Option<WorkTicket<Completion>>,
    stop: Arc<AtomicBool>,
    pub cursor: usize,
    selected: Option<feedback::SelectedMoment>,
    pub mode: ReviewMode,
    pub message: String,
    lesson_return: Option<lessons::LessonReturn>,
    error: Option<String>,
    pub moves: Vec<String>,
    text_links: links::ReviewLinks,
}
pub(super) enum ReviewMode {
    Replay,
    Reply {
        positions: Vec<ChessState>,
        next: Instant,
        index: usize,
    },
    Retry,
}
struct Completion {
    review: Review,
    feedback: feedback::Feedback,
    result: Result<crate::review::ReviewStep, crate::rules::MoveError>,
}
impl ReviewSession {
    fn new(payload: GamePayload, parent: Screen, config: ReviewConfig) -> Result<Self, String> {
        let recorded = payload.replay()?;
        let mut position = ChessState::from_fen(&payload.initial_fen).map_err(|e| e.to_string())?;
        let player = match payload.mode {
            SavedMode::Computer { human, .. } => human,
            SavedMode::TwoPlayer => position.side_to_move(),
        };
        let mut legal = Vec::new();
        let mut moves = Vec::new();
        for (index, uci) in payload.moves.iter().enumerate() {
            let chess_move = position.parse_uci(uci).map_err(|e| e.to_string())?;
            moves.push(format!(
                "{}. {}",
                index + 1,
                position.to_san(chess_move).map_err(|e| e.to_string())?
            ));
            position.play(chess_move).map_err(|e| e.to_string())?;
            legal.push(chess_move);
        }
        let initial = ChessState::from_fen(&payload.initial_fen).map_err(|e| e.to_string())?;
        let core = Review::new(initial, legal, player, config).map_err(|e| e.to_string())?;
        Ok(Self {
            report: core.report().clone(),
            feedback: feedback::Feedback::new(),
            progress: core.progress(),
            core: Some(core),
            cursor: payload.moves.len(),
            payload,
            recorded,
            camera: None,
            parent,
            queue: None,
            pending: None,
            stop: Arc::new(AtomicBool::new(false)),
            selected: None,
            mode: ReviewMode::Replay,
            message: String::new(),
            lesson_return: None,
            error: None,
            moves,
            text_links: links::ReviewLinks::default(),
        })
    }
    fn move_labels(&self) -> Vec<String> {
        self.moves
            .iter()
            .enumerate()
            .map(|(index, text)| {
                let marker = if index + 1 == self.cursor { "> " } else { "" };
                let label = self.report.plies.get(index).map_or(String::new(), |ply| {
                    if ply.label == crate::review::Label::Unclear {
                        " - unclear: analysis cannot judge this move".into()
                    } else {
                        format!(" - {}", ply.label)
                    }
                });
                format!("{marker}{text}{label}")
            })
            .collect()
    }
    fn submit(&mut self) -> Result<(), String> {
        let Some(mut review) = self.core.take() else {
            return Ok(());
        };
        let stop = self.stop.clone();
        let mut feedback = self.feedback.clone();
        // One queued job owns one bounded ply. The UI never waits for search.
        let ticket = self.queue.as_ref().expect("review queue").submit(move |_| {
            let result = review.advance(&stop);
            feedback::update(&review.report().moments, &mut feedback);
            Completion {
                review,
                feedback,
                result,
            }
        });
        match ticket {
            Ok(ticket) => {
                self.pending = Some(ticket);
                Ok(())
            }
            Err(WorkSubmitError::Full) => Err("Review queue unexpectedly full".into()),
            Err(error) => Err(format!("Review error: {error}")),
        }
    }
}
impl BoardDriver {
    pub(super) fn review_ui_state(&self) -> super::ui::ReviewUiState {
        let mut state = super::ui::ReviewUiState::default();
        if let Some(storage) = &self.storage {
            state.finished = std::array::from_fn(|index| {
                storage.finished[index].as_ref().map(|p| {
                    let name = if index == 0 {
                        "Current game".into()
                    } else {
                        format!("Slot {index}")
                    };
                    format!("{name}: {}", p.label())
                })
            });
        }
        if self.screen() != Screen::Review {
            return state;
        }
        let session = self.review.as_ref().expect("session");
        state.moves = session.move_labels();
        state.moments = session
            .report
            .moments
            .iter()
            .map(|m| {
                format!(
                    "Move {}: {}. {}",
                    m.ply, m.label, session.feedback[&m.ply].learner
                )
            })
            .collect();
        state.emphasis = session
            .report
            .moments
            .iter()
            .chain(session.selected.iter().map(|selected| &selected.moment))
            .flat_map(feedback::emphasis)
            .collect();
        state.message.clone_from(&session.message);
        state.related_lesson = self.related_review_lesson().is_some();
        if state.moments.is_empty() {
            state.message.push_str(
                "
No confident teachable moments yet.",
            );
        }
        state
    }
    pub fn review_report(&self) -> Option<&ReviewReport> {
        self.review.as_ref().map(|r| &r.report)
    }
    pub fn review_cursor(&self) -> Option<usize> {
        self.review.as_ref().map(|r| r.cursor)
    }
    pub fn review_pending(&self) -> bool {
        self.review.as_ref().is_some_and(|r| r.pending.is_some())
    }
    pub(super) fn open_review(&mut self, config: ReviewConfig) -> Result<(), String> {
        if !self.game_ended() {
            return Err("only finished games can be reviewed".into());
        }
        let board = self.board.as_ref().expect("finished board");
        let mode = self
            .computer_game()
            .map_or(SavedMode::TwoPlayer, |c| SavedMode::Computer {
                human: c.human(),
                level: c.level(),
            });
        let payload = GamePayload::from_game(
            board.game(),
            board.position().seed,
            mode,
            self.menus.result.clone(),
        );
        if self.review.as_ref().is_none_or(|r| r.payload != payload) {
            self.review = Some(ReviewSession::new(payload, self.screen(), config)?);
        }
        let session = self.review.as_mut().expect("review session");
        session.stop.store(false, Ordering::Relaxed);
        session.mode = ReviewMode::Replay;
        let cursor = session.cursor;
        session.camera = self.camera.take();
        let board = self.board.as_ref().expect("review board");
        let mut options =
            super::viewer_options(board.scene(), crate::CameraPreset::Overhead, self.viewport);
        options.pose = worldrunner::scene::gpu::CameraPose {
            eye_millimeters: [4_000, -2_000, 17_000],
            target_millimeters: [4_000, 0, 0],
        };
        self.camera = Some(super::driver::CameraControls {
            orbit: worldrunner::scene::viewer::OrbitCamera::new(options.pose, options.orbit)
                .map_err(|e| e.to_string())?,
            dirty: true,
        });
        self.seek_review(cursor)?;
        self.transition(Screen::Review)
    }
    pub(super) fn leave_review(&mut self) -> Result<(), String> {
        let session = self.review.as_mut().expect("review session");
        // Review commits an in-flight ply atomically, then pauses. Keep its result
        // ticket alive so resuming retains that work instead of discarding it.
        session.stop.store(true, Ordering::Relaxed);
        session.mode = ReviewMode::Replay;
        self.camera = session.camera.take();
        if let Some(camera) = &mut self.camera {
            camera.dirty = true;
        }
        let recorded = session.recorded.clone();
        self.display_review_position(recorded)
    }
    pub(super) fn display_review_position(&mut self, position: ChessState) -> Result<(), String> {
        self.clear_selection()?;
        let board = self.board.as_mut().expect("review board");
        board.game = position;
        let mut edits = board.synchronize();
        edits.extend(self.markers.clear());
        if !edits.is_empty() {
            self.pending.borrow_mut().push_back(edits);
        }
        Ok(())
    }
    pub(super) fn advance_review(&mut self, host: &mut RetainedHost) -> Result<(), String> {
        self.drain_review();
        if self.screen() != Screen::Review {
            return Ok(());
        }
        self.advance_reply(Instant::now())?;
        let session = self.review.as_mut().expect("review session");
        if session.pending.is_some()
            || session.progress.completed == session.progress.total
            || session.error.is_some()
        {
            return Ok(());
        }
        if session.queue.is_none() {
            session.queue = Some(host.work_queue(1, 1).map_err(|e| e.to_string())?);
        }
        if let Err(error) = session.submit() {
            session.error = Some(error);
        }
        Ok(())
    }
    pub(super) fn drain_review(&mut self) {
        let Some(session) = &mut self.review else {
            return;
        };
        while let Some(completion) = session.queue.as_ref().and_then(WorkQueue::try_recv) {
            if session.pending.as_ref() != Some(&completion.ticket) {
                continue;
            }
            session.pending = None;
            match completion.result {
                Ok(done) => {
                    session.progress = done.review.progress();
                    session.report = done.review.report().clone();
                    session.feedback = done.feedback;
                    session.core = Some(done.review);
                    if let Err(error) = done.result {
                        session.error = Some(format!("Review error: {error}"));
                    }
                }
                Err(error) => session.error = Some(format!("Review error: {error}")),
            }
        }
    }
    pub(super) fn review_status(&self) -> Option<String> {
        if self.screen() != Screen::Review {
            return None;
        }
        let session = self.review.as_ref().expect("review session");
        if let Some(error) = &session.error {
            return Some(error.clone());
        }
        let progress = session.progress;
        let message = session.message.lines().next().unwrap_or("");
        Some(if progress.completed == progress.total {
            format!("Review complete: {} moves\n{}", progress.total, message)
        } else {
            format!(
                "Analysing move {} of {}\n{}",
                progress.completed + 1,
                progress.total,
                message
            )
        })
    }
    pub(super) fn retry_active(&self) -> bool {
        self.screen() == Screen::Review
            && self
                .review
                .as_ref()
                .is_some_and(|r| matches!(r.mode, ReviewMode::Retry))
    }
    pub(super) fn review_action(&mut self, action: MenuAction) -> Result<(), String> {
        self.handle_review_action(action)
    }
    pub(super) fn review_move(&mut self, chess_move: LegalMove) -> Result<(), String> {
        self.try_review_move(chess_move)
    }
}
