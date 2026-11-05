use crate::{
    review::TeachableMoment,
    teaching::{Course, MomentExplanation, explain::explain_moment},
};
use std::collections::BTreeMap;

pub(super) struct SelectedMoment {
    pub moment: TeachableMoment,
    pub explanation: MomentExplanation,
}

pub(super) type Feedback = BTreeMap<usize, MomentExplanation>;

pub(super) fn emphasis(moment: &TeachableMoment) -> Vec<String> {
    let state = &moment.position_before;
    let mut moves = vec![
        state.to_san(moment.played).expect("review move"),
        state.to_san(moment.better).expect("review alternative"),
    ];
    moves.extend(moment.refutation.san.iter().cloned());
    moves
}

pub(super) fn update(moments: &[TeachableMoment], feedback: &mut Feedback) {
    feedback.retain(|ply, _| moments.iter().any(|moment| moment.ply == *ply));
    let course = Course::bundled().expect("bundled course is validated");
    for moment in moments {
        feedback.entry(moment.ply).or_insert_with(|| {
            let mut explanation = explain_moment(moment);
            if let Some(lesson) = explanation
                .motif
                .and_then(|motif| course.related_lesson(motif))
            {
                let link = format!(
                    "\nRelated lesson: {} ({}) - open from Lessons.",
                    lesson.title, lesson.id
                );
                explanation.learner.push_str(&link);
                explanation.detail.push_str(&link);
            }
            explanation
        });
    }
}
