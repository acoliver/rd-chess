#[cfg(test)]
mod tests;

impl super::NativeUi {
    pub(super) fn fit_scroll_paragraph(
        &mut self,
        object: ObjectHandle,
        height: f32,
    ) -> Result<(), WorldError> {
        let parent = self
            .world
            .parent(object)?
            .filter(|parent| self.router.controls.contains_key(parent));
        for object in [Some(object), parent].into_iter().flatten() {
            let handle = self.rect_handle(object);
            let ComponentDesc::RectTransform(mut transform) = self.world.component(handle)?.desc
            else {
                unreachable!("paragraph rectangle");
            };
            transform.size_delta.y = height;
            self.world
                .set_component(handle, ComponentDesc::RectTransform(transform))?;
        }
        Ok(())
    }
}

use super::{GameUi, MenuAction};
use crate::teaching::Course;
use std::{collections::BTreeMap, ops::Range};
use worldrunner::{
    simulation::{ComponentDesc, ComponentHandle, ObjectHandle, WorldError},
    ui::{TextRun, TextSpan, UiControl, UiLinkId, UiStyle},
};

pub(in crate::retained) type References = Vec<(String, MenuAction)>;
pub(in crate::retained) type ReviewReferences = BTreeMap<String, References>;

#[derive(Default)]
pub(super) struct Links {
    course: References,
    review: ReviewReferences,
    actions: BTreeMap<(ObjectHandle, UiLinkId), MenuAction>,
    children: BTreeMap<ComponentHandle, ComponentHandle>,
}

impl GameUi {
    pub(in crate::retained) fn set_course_links(&mut self, course: &Course) {
        self.links.course = course
            .lessons()
            .iter()
            .enumerate()
            .flat_map(|(index, lesson)| {
                lesson
                    .terms
                    .iter()
                    .chain([&lesson.id, &lesson.title])
                    .map(move |term| (term.clone(), MenuAction::LinkedLesson(index)))
            })
            .collect();
    }

    pub(in crate::retained) fn set_review_links(&mut self, references: ReviewReferences) {
        self.links.review = references;
    }

    pub(super) fn link_action(&self, action: worldrunner::ui::UiAction) -> Option<MenuAction> {
        let worldrunner::ui::UiAction::LinkActivated { control, link } = action else {
            return None;
        };
        self.links.actions.get(&(control, link)).copied()
    }

    pub(super) fn link_paragraph(
        &mut self,
        handle: ComponentHandle,
        run: &mut TextRun,
        heading_end: usize,
    ) -> Result<ComponentHandle, WorldError> {
        let references = self
            .links
            .review
            .get(&run.text)
            .into_iter()
            .flatten()
            .chain(self.links.course.iter())
            .collect::<Vec<_>>();
        let matches = matches(&run.text, heading_end, &references);
        if matches.is_empty() && !self.links.children.contains_key(&handle) {
            return Ok(handle);
        }
        let handle = self.link_control(handle)?;
        let object = self.native.world.component(handle)?.object;
        self.links
            .actions
            .retain(|(control, _), _| *control != object);
        self.native
            .router
            .link_styles
            .retain(|(control, _), _| *control != object);
        for (index, (range, action)) in matches.into_iter().enumerate() {
            let id = UiLinkId(u32::try_from(index).expect("bounded paragraph"));
            run.spans
                .retain(|span| span.range.end <= range.start || span.range.start >= range.end);
            run.spans.push(TextSpan {
                range,
                color: [1.; 4],
                link: Some(id),
            });
            self.links.actions.insert((object, id), action);
            self.native
                .router
                .link_styles
                .insert((object, id), link_style());
        }
        run.spans.sort_by_key(|span| span.range.start);
        self.native
            .router
            .controls
            .entry(object)
            .or_insert_with(UiControl::text);
        Ok(handle)
    }

    fn link_control(&mut self, handle: ComponentHandle) -> Result<ComponentHandle, WorldError> {
        if let Some(child) = self.links.children.get(&handle) {
            return Ok(*child);
        }
        let object = self.native.world.component(handle)?.object;
        if self
            .native
            .router
            .controls
            .get(&object)
            .is_none_or(|control| control.kind == worldrunner::ui::UiControlKind::Text)
        {
            return Ok(handle);
        }
        let rect = self.native.rect_handle(object);
        let ComponentDesc::RectTransform(transform) = self.native.world.component(rect)?.desc
        else {
            unreachable!("paragraph rectangle");
        };
        let child = self
            .native
            .text(object, "", transform.size_delta.to_array(), [0., 0.])?;
        let child = self.native.text_handle(child);
        self.native.set_text(handle, "")?;
        self.links.children.insert(handle, child);
        Ok(child)
    }
}

fn matches(
    text: &str,
    start: usize,
    references: &[&(String, MenuAction)],
) -> Vec<(Range<usize>, MenuAction)> {
    let mut matches = Vec::new();
    let mut occurrences = BTreeMap::new();
    let mut offset = start;
    while offset < text.len() {
        let found = references
            .iter()
            .filter(|(word, _)| reference_at(text, offset, word))
            .max_by_key(|(word, _)| word.len());
        if let Some((word, _)) = found {
            let candidates: Vec<_> = references
                .iter()
                .filter(|(candidate, _)| candidate.eq_ignore_ascii_case(word))
                .collect();
            let count = occurrences
                .entry(word.to_ascii_lowercase())
                .or_insert(0_usize);
            let action = candidates[(*count).min(candidates.len() - 1)].1;
            *count += 1;
            matches.push((offset..offset + word.len(), action));
            offset += word.len();
        } else {
            offset += text[offset..].chars().next().expect("character").len_utf8();
        }
    }
    matches
}

fn reference_at(text: &str, offset: usize, word: &str) -> bool {
    let end = offset + word.len();
    text.get(offset..end)
        .is_some_and(|part| part.eq_ignore_ascii_case(word))
        && (offset == 0 || !word_character(text.as_bytes()[offset - 1]))
        && (end == text.len() || !word_character(text.as_bytes()[end]))
}
fn word_character(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || b"_+#=-".contains(&byte)
}
fn link_style() -> UiStyle {
    UiStyle {
        normal: [0.45, 0.9, 1., 1.],
        focused: [1., 0.72, 0.2, 1.],
        hovered: [0.7, 0.85, 1., 1.],
        pressed: [1., 0.5, 0.1, 1.],
        ..UiStyle::default()
    }
}
