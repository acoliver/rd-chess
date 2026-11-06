use super::{BoardDriver, Screen, menus::MenuAction};

impl BoardDriver {
    pub(super) fn lesson_lock_message(&self, index: usize) -> Option<String> {
        let training = self.training.as_ref().expect("training");
        let lesson = &training.course.lessons()[index];
        let missing: Vec<_> = lesson
            .requires
            .iter()
            .filter(|id| !training.progress.completed_lessons.contains(*id))
            .map(|id| {
                let prerequisite = training.course.lesson(id).expect("validated prerequisite");
                format!("{} ({})", prerequisite.title, prerequisite.id)
            })
            .collect();
        (!missing.is_empty()).then(|| {
            format!(
                "{} ({}) is locked. Complete {} to unlock it.",
                lesson.title,
                lesson.id,
                missing.join(", ")
            )
        })
    }

    pub(super) fn open_text_lesson(&mut self, index: usize) -> Result<(), String> {
        if let Some(message) = self.lesson_lock_message(index) {
            if self.screen() == Screen::Review {
                self.review.as_mut().expect("review session").message = message;
            } else {
                let training = self.training.as_mut().expect("training");
                training.panel = format!("Lesson locked\n{message}");
            }
            return self.sync_menu_ui();
        }
        self.persist_training_progress();
        if self.screen() == Screen::Review {
            self.open_review_lesson_at(index)
        } else {
            self.training_action(MenuAction::Lesson(index))
        }
    }
}
