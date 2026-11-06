use super::{BoardDriver, Screen};
use crate::{
    Square,
    rules::LegalMove,
    teaching::{AnnotationPalette, Annotations},
};
use glam::{Vec2, Vec3};
use worldrunner::ui::{
    TextRun,
    plan::ui::annotations::{Annotation, AnnotationDepth, AnnotationPoint, AnnotationPrimitive},
};

impl BoardDriver {
    /// The same world-anchored primitives consumed by the native UI publisher.
    pub fn board_annotations(&self) -> Vec<Annotation> {
        match self.screen() {
            Screen::Review => self.review_annotations(),
            Screen::Lesson | Screen::Puzzle | Screen::Play => self
                .training
                .as_ref()
                .map_or_else(Vec::new, |t| t.annotations.clone()),
            _ => Vec::new(),
        }
    }
}
pub(super) fn point(square: Square) -> AnnotationPoint {
    AnnotationPoint::World(Vec3::new(
        f32::from(square.file) - 3.5,
        f32::from(square.rank) - 3.5,
        0.065,
    ))
}
pub(super) fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').expect("validated annotation")
}
fn annotation(primitive: AnnotationPrimitive) -> Annotation {
    Annotation {
        primitive,
        draw_order: (-1, 0, 0),
        clip_rect: None,
        depth: AnnotationDepth::DepthTested,
    }
}
pub(super) fn ring(square: Square, colour: [f32; 4]) -> Annotation {
    annotation(AnnotationPrimitive::Circle {
        centre: point(square),
        radius: 20.,
        thickness: Some(3.),
        colour,
        segments: 64,
    })
}
pub(super) fn arrow(from: Square, to: Square, colour: [f32; 4]) -> Annotation {
    annotation(AnnotationPrimitive::Arrow {
        from: point(from),
        to: point(to),
        width: 5.,
        head_length: 17.,
        head_width: 19.,
        colour,
    })
}
pub(super) fn authored(data: &Annotations) -> Vec<Annotation> {
    let mut annotations: Vec<_> = data
        .squares
        .iter()
        .map(|name| ring(square(name), data.square_colour))
        .collect();
    annotations.extend(
        data.arrows
            .iter()
            .map(|a| arrow(square(&a.from), square(&a.to), a.colour)),
    );
    annotations
}
pub(super) fn labelled_move(
    mv: LegalMove,
    colour: [f32; 4],
    text: &str,
    font: worldrunner::ui::FontId,
) -> Vec<Annotation> {
    let mut run = TextRun::new(font);
    run.text = text.into();
    run.font_size = 14.;
    run.color = colour;
    vec![
        arrow(mv.from(), mv.to(), colour),
        annotation(AnnotationPrimitive::Label {
            anchor: point(mv.to()),
            run,
            alignment: Vec2::new(0.5, 0.),
        }),
    ]
}
impl BoardDriver {
    pub(super) fn explanation_annotations(
        &self,
        suggestion: Option<LegalMove>,
        threat: Option<LegalMove>,
    ) -> Vec<Annotation> {
        let AnnotationPalette {
            suggestion: blue,
            threat: orange,
        } = self.training.as_ref().expect("training").palette;
        let font = self.ui.as_ref().expect("configured UI").annotation_font();
        let mut annotations = Vec::new();
        if let Some(mv) = suggestion {
            annotations.extend(labelled_move(mv, blue, "Better", font));
        }
        if let Some(mv) = threat {
            annotations.extend(labelled_move(mv, orange, "Threat", font));
        }
        annotations
    }
}

#[cfg(test)]
mod tests;
