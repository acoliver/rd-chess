use super::{BoardDriver, Ordering, ReviewMode, Screen};
use crate::{
    retained::{PieceObjects, computer::Computer, menus::MenuAction},
    teaching::Lesson,
};
use std::collections::BTreeMap;

pub(super) struct LessonReturn {
    identities: BTreeMap<String, PieceObjects>,
    computer: Option<Computer>,
    result: Option<String>,
}

impl BoardDriver {
    pub(in crate::retained) fn related_review_lesson(&self) -> Option<&Lesson> {
        let motif = self.review.as_ref()?.selected.as_ref()?.explanation.motif?;
        self.training.as_ref()?.course.related_lesson(motif)
    }
    pub(in crate::retained) fn review_lesson_active(&self) -> bool {
        self.review
            .as_ref()
            .is_some_and(|review| review.lesson_return.is_some())
    }
    pub(super) fn open_review_lesson(&mut self) -> Result<(), String> {
        let Some(lesson) = self.related_review_lesson() else {
            return Ok(());
        };
        let training = self.training.as_ref().expect("training");
        let index = training
            .course
            .lessons()
            .iter()
            .position(|candidate| candidate.id == lesson.id)
            .expect("related lesson");
        self.open_text_lesson(index)
    }
    pub(in crate::retained) fn open_review_lesson_at(
        &mut self,
        index: usize,
    ) -> Result<(), String> {
        let session = self.review.as_mut().expect("session");
        session.stop.store(true, Ordering::Relaxed);
        session.mode = ReviewMode::Replay;
        // Study replaces rules piece identities. Keep the review's mapping so its
        // recorded positions and retained object IDs still agree on return.
        session.lesson_return = Some(LessonReturn {
            identities: self
                .board
                .as_ref()
                .expect("review board")
                .identities
                .clone(),
            computer: self.computer.take(),
            result: self.menus.result.clone(),
        });
        self.training_action(MenuAction::Lesson(index))
    }
    pub(in crate::retained) fn return_from_review_lesson(&mut self) -> Result<(), String> {
        let orbit = self
            .training
            .as_mut()
            .expect("training")
            .leave()
            .expect("study camera");
        let camera = self.camera.as_mut().expect("configured camera");
        camera.orbit = orbit;
        camera.dirty = true;
        let session = self.review.as_mut().expect("session");
        let saved = session.lesson_return.take().expect("linked lesson return");
        session.stop.store(false, Ordering::Relaxed);
        let cursor = session.cursor;
        self.computer = saved.computer;
        self.menus.result = saved.result;
        self.board.as_mut().expect("review board").identities = saved.identities;
        self.seek_review(cursor)?;
        self.transition(Screen::Review)
    }
}
