use super::{BoardDriver, ChessState, ReviewMode, ReviewSession, Screen};
use crate::{
    retained::{
        menus::MenuAction,
        ui::{References, ReviewReferences, ReviewUiState},
    },
    review::TeachableMoment,
};

#[derive(Default)]
pub(super) struct ReviewLinks {
    positions: Vec<(usize, ChessState)>,
    state: Option<ReviewUiState>,
    references: ReviewReferences,
}

impl ReviewSession {
    fn text_target(&mut self, ply: usize, position: ChessState) -> MenuAction {
        let index = self.text_links.positions.len();
        self.text_links.positions.push((ply, position));
        MenuAction::ReviewText(index)
    }

    fn moment_links(&mut self, moment: &TeachableMoment) -> References {
        let mut position = moment.position_before.clone();
        let san = position.to_san(moment.played).expect("reviewed move");
        position.play(moment.played).expect("reviewed move");
        let mut links = vec![(san, self.text_target(moment.ply, position.clone()))];
        for (offset, &mv) in moment.refutation.moves.iter().enumerate() {
            let san = position.to_san(mv).expect("searched reply");
            position.play(mv).expect("searched reply");
            links.push((
                san,
                self.text_target(moment.ply + offset + 1, position.clone()),
            ));
        }
        let mut alternative = moment.position_before.clone();
        let san = alternative
            .to_san(moment.better)
            .expect("searched alternative");
        alternative
            .play(moment.better)
            .expect("searched alternative");
        links.push((san, self.text_target(moment.ply, alternative)));
        links
    }

    fn recorded_links(&mut self, labels: &[String]) -> ReviewReferences {
        let mut links = ReviewReferences::new();
        let mut position = ChessState::from_fen(&self.payload.initial_fen).expect("review record");
        let moves = self.payload.moves.clone();
        for (index, (uci, label)) in moves.iter().zip(labels).enumerate() {
            let mv = position.parse_uci(uci).expect("review record");
            let san = position.to_san(mv).expect("review record");
            position.play(mv).expect("review record");
            links.insert(
                label.clone(),
                vec![(san, self.text_target(index + 1, position.clone()))],
            );
        }
        links
    }
}

impl BoardDriver {
    pub(in crate::retained) fn review_text_links(
        &mut self,
        state: &ReviewUiState,
    ) -> ReviewReferences {
        if self.screen() != Screen::Review {
            return ReviewReferences::new();
        }
        let session = self.review.as_mut().expect("review session");
        if session.text_links.state.as_ref() == Some(state) {
            return session.text_links.references.clone();
        }
        session.text_links.positions.clear();
        let mut links = session.recorded_links(&state.moves);
        let moments = session.report.moments.clone();
        for (moment, text) in moments.iter().zip(&state.moments) {
            let references = session.moment_links(moment);
            links.insert(text.clone(), references);
        }
        if let Some(moment) = session
            .selected
            .as_ref()
            .map(|selected| selected.moment.clone())
        {
            let references = session.moment_links(&moment);
            links.insert(state.message.clone(), references);
        }
        session.text_links.state = Some(state.clone());
        session.text_links.references.clone_from(&links);
        links
    }

    pub(super) fn follow_review_text(&mut self, index: usize) -> Result<(), String> {
        let session = self.review.as_mut().expect("review session");
        let (ply, position) = session.text_links.positions[index].clone();
        session.cursor = ply;
        session.mode = ReviewMode::Replay;
        self.display_review_position(position)
    }
}
