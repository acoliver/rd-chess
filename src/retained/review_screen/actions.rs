use super::{BoardDriver, Duration, Instant, ReviewMode, Screen};
use crate::{
    retained::menus::MenuAction,
    review::ReviewConfig,
    rules::{ChessState, LegalMove},
};

impl BoardDriver {
    pub(super) fn handle_review_action(&mut self, action: MenuAction) -> Result<(), String> {
        match action {
            MenuAction::Review => self.open_review(ReviewConfig::default())?,
            MenuAction::ReviewGames => self.transition(Screen::ReviewGames)?,
            MenuAction::ReviewSaved(index) => self.open_saved_review(index)?,
            MenuAction::ReviewSeek(ply) => self.seek_review(ply)?,
            MenuAction::ReviewStart => self.seek_review(0)?,
            MenuAction::ReviewBack => {
                let ply = self
                    .review
                    .as_ref()
                    .expect("session")
                    .cursor
                    .saturating_sub(1);
                self.seek_review(ply)?;
            }
            MenuAction::ReviewForward => {
                let ply = self.review.as_ref().expect("session").cursor + 1;
                self.seek_review(ply)?;
            }
            MenuAction::ReviewEnd => {
                let ply = self.review.as_ref().expect("session").payload.moves.len();
                self.seek_review(ply)?;
            }
            MenuAction::Moment(index) => {
                let session = self.review.as_mut().expect("session");
                let moment = session.report.moments[index].clone();
                session.message = crate::teaching::explain::explain_moment(&moment).learner;
                let ply = moment.ply;
                session.selected = Some(moment);
                self.seek_review(ply)?;
            }
            MenuAction::WhatChanged => self.explain_review(),
            MenuAction::ShowReply => self.show_review_reply()?,
            MenuAction::TryAgain => self.retry_review()?,
            MenuAction::ReviewReturn => {
                let ply = self.review.as_ref().expect("session").cursor;
                self.seek_review(ply)?;
            }
            MenuAction::ReviewExit => {
                let parent = self.review.as_ref().expect("session").parent;
                self.transition(parent)?;
            }
            _ => unreachable!("review action"),
        }
        self.sync_menu_ui()
    }
    pub(super) fn seek_review(&mut self, ply: usize) -> Result<(), String> {
        let session = self.review.as_mut().expect("review session");
        session.cursor = ply.min(session.payload.moves.len());
        session.mode = ReviewMode::Replay;
        let mut state =
            ChessState::from_fen(&session.payload.initial_fen).map_err(|e| e.to_string())?;
        for uci in &session.payload.moves[..session.cursor] {
            let chess_move = state.parse_uci(uci).map_err(|e| e.to_string())?;
            state.play(chess_move).map_err(|e| e.to_string())?;
        }
        self.display_review_position(state)
    }
    fn explain_review(&mut self) {
        let session = self.review.as_mut().expect("session");
        if let Some(moment) = &session.selected {
            session.message = crate::teaching::explain::explain_moment(moment).detail;
        }
    }
    fn retry_review(&mut self) -> Result<(), String> {
        let session = self.review.as_mut().expect("session");
        let Some(moment) = &session.selected else {
            return Ok(());
        };
        let position = moment.position_before.clone();
        session.mode = ReviewMode::Retry;
        session.message = "Try a better move. Return to line leaves the record unchanged.".into();
        self.display_review_position(position)
    }
    pub(super) fn try_review_move(&mut self, chess_move: LegalMove) -> Result<(), String> {
        let session = self.review.as_mut().expect("session");
        if !matches!(session.mode, ReviewMode::Retry) {
            return Err("not practising a review move".into());
        }
        let moment = session.selected.as_ref().expect("practice moment");
        if chess_move != moment.better {
            session.message = format!(
                "Try again. {}",
                crate::teaching::explain::explain_moment(moment).detail
            );
            return self.clear_selection();
        }
        let mut state = moment.position_before.clone();
        state.play(chess_move).map_err(|e| e.to_string())?;
        session.message =
            "Yes: that is the searched better move. Return to line to continue reviewing.".into();
        session.mode = ReviewMode::Replay;
        self.display_review_position(state)
    }
    fn show_review_reply(&mut self) -> Result<(), String> {
        let session = self.review.as_mut().expect("session");
        let Some(moment) = &session.selected else {
            return Ok(());
        };
        let mut state = moment.position_before.clone();
        state.play(moment.played).map_err(|e| e.to_string())?;
        let mut positions = vec![state.clone()];
        for &chess_move in &moment.refutation.moves {
            state.play(chess_move).map_err(|e| e.to_string())?;
            positions.push(state.clone());
        }
        let first = positions[0].clone();
        session.message = "Showing the searched reply, then returning to the recorded line.".into();
        session.mode = ReviewMode::Reply {
            positions,
            index: 0,
            next: Instant::now() + Duration::from_millis(650),
        };
        self.display_review_position(first)
    }
    pub(super) fn advance_reply(&mut self, now: Instant) -> Result<(), String> {
        let session = self.review.as_mut().expect("session");
        let ReviewMode::Reply {
            positions,
            index,
            next,
        } = &mut session.mode
        else {
            return Ok(());
        };
        if now < *next || !self.pending.borrow().is_empty() || self.presenting {
            return Ok(());
        }
        *index += 1;
        *next = now + Duration::from_millis(650);
        if let Some(position) = positions.get(*index) {
            let position = position.clone();
            self.display_review_position(position)
        } else {
            let cursor = session.cursor;
            self.seek_review(cursor)
        }
    }
    fn open_saved_review(&mut self, index: u8) -> Result<(), String> {
        let payload = self
            .storage
            .as_ref()
            .and_then(|s| s.finished[usize::from(index)].clone())
            .ok_or("no finished saved game")?;
        let storage = self.storage.take();
        let result = self.restore_game(payload);
        self.storage = storage;
        result?;
        self.open_review(ReviewConfig::default())?;
        self.review.as_mut().expect("session").parent = Screen::ReviewGames;
        Ok(())
    }
}
