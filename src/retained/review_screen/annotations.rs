use super::{BoardDriver, ReviewMode};
use worldrunner::ui::plan::ui::annotations::Annotation;

impl BoardDriver {
    pub(in crate::retained) fn review_annotations(&self) -> Vec<Annotation> {
        let session = self.review.as_ref().expect("review session");
        let Some(selected) = &session.selected else {
            return Vec::new();
        };
        if session.cursor != selected.moment.ply || !matches!(session.mode, ReviewMode::Replay) {
            return Vec::new();
        }
        let moment = &selected.moment;
        self.explanation_annotations(
            Some(moment.better),
            moment.refutation.moves.first().copied(),
        )
    }
}
