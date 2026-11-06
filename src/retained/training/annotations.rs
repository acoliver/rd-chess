use super::{Session, Setup, Step, Training};
use crate::retained::annotations::{arrow, authored, ring, square};

impl Training {
    pub(in crate::retained) fn annotation_description(
        &self,
        game: &crate::rules::ChessState,
    ) -> String {
        let (data, hint_move) = match self.session.as_ref() {
            Some(Session::Lesson { runner, .. }) => {
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
            Some(Session::Puzzle {
                puzzle,
                runner,
                setup: Setup::Ready,
            }) => (&puzzle.annotations, runner.hint_move()),
            _ => return String::new(),
        };
        let mut words = Vec::new();
        if !data.squares.is_empty() {
            words.push(format!("Highlighted: {}", data.squares.join(", ")));
        }
        words.extend(
            data.arrows
                .iter()
                .map(|a| format!("Arrow: {} to {}", a.from, a.to)),
        );
        if let Some(mv) = hint_move {
            let from = crate::retained::announcements::square_name(mv.from());
            if self.hint_level >= 2 {
                let (_, kind) = game.piece_at(mv.from()).expect("hint mover");
                words.push(format!(
                    "Hint: {} on {from}",
                    crate::retained::announcements::piece_name(kind)
                ));
            }
            if self.hint_level >= 3 {
                words.push(format!(
                    "Suggested: {from} to {}",
                    crate::retained::announcements::square_name(mv.to())
                ));
            }
        }
        words.join(". ")
    }
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
