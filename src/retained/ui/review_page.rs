#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::{GameUi, MenuAction, Screen, UiState};
use worldrunner::{
    simulation::{ComponentDesc, ObjectHandle, WorldError},
    ui::{RectTransform, UiAxis, UiControl, UiScrollView, UiSize},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub(in crate::retained) struct ReviewUiState {
    pub moves: Vec<String>,
    pub moments: Vec<String>,
    pub message: String,
    pub finished: [Option<String>; 4],
}
#[derive(Default)]
pub(super) struct ReviewWidgets {
    moves: Option<ObjectHandle>,
    move_rows: Vec<ObjectHandle>,
    moments: Vec<ObjectHandle>,
    message: Option<ObjectHandle>,
}
impl GameUi {
    pub(super) fn review_pages(&mut self) -> Result<(), WorldError> {
        self.review_browser()?;
        let panel = self.native.rect(self.native.root, [310., 490.], [0., 0.])?;
        self.native.background(panel, [310., 490.])?;
        let handle = self.native.rect_handle(panel);
        self.native.world.set_component(
            handle,
            ComponentDesc::RectTransform(RectTransform {
                anchor_min: glam::Vec2::new(1., 0.5),
                anchor_max: glam::Vec2::new(1., 0.5),
                anchored_position: glam::Vec2::new(-163., -30.),
                size_delta: glam::Vec2::new(310., 490.),
                ..RectTransform::default()
            }),
        )?;
        self.pages.push((Screen::Review, panel));
        self.native.text(panel, "Review", [290., 28.], [0., 225.])?;
        self.review_row(
            panel,
            190.,
            &[
                ("Start", MenuAction::ReviewStart),
                ("Back", MenuAction::ReviewBack),
                ("Next", MenuAction::ReviewForward),
                ("End", MenuAction::ReviewEnd),
            ],
        )?;
        let list = self.review_scroll(panel, [290., 94.], [0., 122.])?;
        self.review_widgets.moves = Some(list);
        self.bindings.insert(list, Vec::new());
        let moments = self.review_scroll(panel, [290., 150.], [0., -6.])?;
        for index in 0..3 {
            let object = self.native.text(moments, "", [290., 100.], [0., 0.])?;
            self.native
                .control(object, UiControl::list_item(moments, index));
            self.review_widgets.moments.push(object);
        }
        self.bindings
            .insert(moments, (0..3).map(MenuAction::Moment).collect());
        self.review_row(
            panel,
            -100.,
            &[
                ("What changed?", MenuAction::WhatChanged),
                ("Show the reply", MenuAction::ShowReply),
            ],
        )?;
        self.review_row(
            panel,
            -137.,
            &[
                ("Try again", MenuAction::TryAgain),
                ("Return to line", MenuAction::ReviewReturn),
            ],
        )?;
        let text = self.review_scroll(panel, [290., 46.], [0., -179.])?;
        self.review_widgets.message = Some(self.native.text(text, "", [290., 0.], [0., 0.])?);
        self.review_row(panel, -222., &[("Leave review", MenuAction::ReviewExit)])
    }
    fn review_browser(&mut self) -> Result<(), WorldError> {
        let panel = self.native.panel([620., 450.])?;
        self.pages.push((Screen::ReviewGames, panel));
        self.native
            .text(panel, "Review finished games", [580., 36.], [0., 195.])?;
        for index in 0..4 {
            self.review_row(
                panel,
                130. - f32::from(index) * 70.,
                &[("", MenuAction::ReviewSaved(index))],
            )?;
        }
        self.review_row(panel, -190., &[("Back", MenuAction::Back)])
    }
    fn review_row(
        &mut self,
        parent: ObjectHandle,
        y: f32,
        entries: &[(&str, MenuAction)],
    ) -> Result<(), WorldError> {
        let width = if self.pages.last().expect("page").0 == Screen::Review {
            290.
        } else {
            580.
        };
        let list = self.native.rect(parent, [width, 32.], [0., y])?;
        self.native.stack(list, UiAxis::Horizontal, 4);
        let count = f32::from(u16::try_from(entries.len()).expect("small row"));
        self.list_rows(list, entries, [(width - (count - 1.) * 4.) / count, 32.])
    }
    fn review_scroll(
        &mut self,
        parent: ObjectHandle,
        size: [f32; 2],
        at: [f32; 2],
    ) -> Result<ObjectHandle, WorldError> {
        let viewport = self.native.rect(parent, size, at)?;
        let list = self.native.rect(viewport, [size[0], 0.], [0., 0.])?;
        self.native.stack(list, UiAxis::Vertical, 2);
        self.native
            .router
            .layout
            .sizes
            .insert(list, UiSize::Content);
        self.native
            .router
            .layout
            .scrolls
            .insert(viewport, UiScrollView::new(list));
        Ok(list)
    }
    pub(super) fn sync_review_controls(&mut self, state: &UiState) -> Result<(), WorldError> {
        self.sync_review_moves(&state.review.moves)?;
        for (index, object) in self.review_widgets.moments.clone().into_iter().enumerate() {
            let label = state.review.moments.get(index);
            self.native.world.set_active(object, label.is_some())?;
            if let Some(label) = label {
                self.review_text(object, label)?;
            }
        }
        if let Some(object) = self.review_widgets.message {
            self.review_text(object, &state.review.message)?;
        }
        let rows: Vec<_> = self
            .rows
            .iter()
            .flat_map(|(list, rows)| {
                rows.iter()
                    .copied()
                    .zip(self.bindings[list].iter().copied())
            })
            .collect();
        for (object, action) in rows {
            if let MenuAction::ReviewSaved(index) = action {
                let label = &state.review.finished[usize::from(index)];
                self.native
                    .router
                    .controls
                    .get_mut(&object)
                    .expect("review save control")
                    .enabled = label.is_some();
                self.replace_text(object, label.as_deref().unwrap_or("No finished game"))?;
            }
        }
        Ok(())
    }
    fn review_text(&mut self, object: ObjectHandle, text: &str) -> Result<(), WorldError> {
        let text = wrap(text, 25);
        let lines = u16::try_from(text.lines().count()).expect("bounded review text");
        let handle = self.native.rect_handle(object);
        let ComponentDesc::RectTransform(mut rect) =
            self.native.world.component(handle)?.desc.clone()
        else {
            unreachable!()
        };
        rect.size_delta.y = f32::from(lines.max(1)) * 24.;
        self.native
            .world
            .set_component(handle, ComponentDesc::RectTransform(rect))?;
        self.replace_text(object, &text)
    }
    fn sync_review_moves(&mut self, labels: &[String]) -> Result<(), WorldError> {
        let list = self.review_widgets.moves.expect("review move list");
        while self.review_widgets.move_rows.len() < labels.len() {
            let index = self.review_widgets.move_rows.len();
            let object = self.native.text(list, "", [290., 28.], [0., 0.])?;
            self.native
                .control(object, UiControl::list_item(list, index));
            self.bindings
                .get_mut(&list)
                .expect("move bindings")
                .push(MenuAction::ReviewSeek(index + 1));
            self.review_widgets.move_rows.push(object);
        }
        for (index, object) in self
            .review_widgets
            .move_rows
            .clone()
            .into_iter()
            .enumerate()
        {
            let label = labels.get(index);
            self.native.world.set_active(object, label.is_some())?;
            if let Some(label) = label {
                self.review_text(object, label)?;
            }
        }
        Ok(())
    }
}
fn wrap(text: &str, width: usize) -> String {
    let mut lines = Vec::new();
    for paragraph in text.lines() {
        let mut line = String::new();
        for word in paragraph.split_whitespace() {
            if !line.is_empty() && line.len() + word.len() + 1 > width {
                lines.push(std::mem::take(&mut line));
            }
            if !line.is_empty() {
                line.push(' ');
            }
            line.push_str(word);
        }
        lines.push(line);
    }
    lines.join("\n")
}
