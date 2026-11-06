use super::{Session, Setup, Step, Training};
use crate::retained::annotations::{arrow, authored, ring, square};

impl Training {
    pub(super) fn refresh_annotations(&mut self) {
        self.highlights.clear();
        self.annotations.clear();
        let (data, hint_move) = match self.session.as_ref().expect("active study") {
            Session::Lesson { runner, .. } => {
                let data = match runner.step().expect("active step") {
                    Step::Explain { annotations, .. } => annotations,
                    Step::Task(task) => &task.annotations,
                };
                (
                    data,
                    runner
                        .task()
                        .and_then(crate::teaching::TaskRunner::hint_move),
                )
            }
            Session::Puzzle {
                puzzle,
                runner,
                setup: Setup::Ready,
            } => (&puzzle.annotations, runner.hint_move()),
            Session::Puzzle { .. } => return,
        };
        self.annotations = authored(data);
        self.highlights
            .extend(data.squares.iter().map(|name| square(name)));
        if let Some(mv) = hint_move {
            if self.hint_level >= 2 {
                self.highlights.insert(mv.from());
                self.annotations
                    .push(ring(mv.from(), self.palette.suggestion));
            }
            if self.hint_level >= 3 {
                self.annotations
                    .push(arrow(mv.from(), mv.to(), self.palette.suggestion));
            }
        }
    }
}
