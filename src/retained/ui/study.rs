#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use super::{GameUi, MenuAction, Screen};
use worldrunner::{
    simulation::{ComponentHandle, ObjectHandle, WorldError},
    ui::{UiAxis, UiControl, UiScrollView, UiSize},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::retained) struct StudyState {
    pub lessons: Vec<(String, bool)>,
    pub bands: [String; 4],
    pub panel: String,
    pub emphasis: Vec<String>,
    pub controls: [bool; 5],
    pub why: bool,
}
pub(super) struct StudyWidgets {
    lessons: Vec<ObjectHandle>,
    bands: Vec<ObjectHandle>,
    panel: ObjectHandle,
    text: ComponentHandle,
    controls: Vec<ObjectHandle>,
    commands: ObjectHandle,
    why: ObjectHandle,
}
impl GameUi {
    pub(super) fn build_study(&mut self) -> Result<(), WorldError> {
        let lessons = self.lesson_browser()?;
        let bands = self.puzzle_browser()?;
        let panel = self.native.rect(self.native.root, [760., 180.], [0., 0.])?;
        self.native.bottom_rect(panel, [0., -24.], [0., 180.])?;
        self.native.background(panel, [760., 180.])?;
        let text = study_text(&mut self.native, panel)?;
        let commands = self.native.rect(panel, [760., 36.], [0., -64.])?;
        self.native.stack(commands, UiAxis::Horizontal, 8);
        let actions = [
            ("Back", MenuAction::StudyBack),
            ("Next", MenuAction::StudyNext),
            ("Hint", MenuAction::StudyHint),
            ("Retry", MenuAction::StudyRetry),
            ("Exit", MenuAction::StudyExit),
        ];
        self.list_rows(commands, &actions, [144., 36.])?;
        let controls = self.rows[&commands].clone();
        let pause = self
            .pages
            .iter()
            .find(|(screen, _)| *screen == Screen::InGameMenu)
            .expect("pause page")
            .1;
        let why = self.native.text(pause, "Why?", [120., 32.], [0., -208.])?;
        self.native.control(why, UiControl::button());
        self.study = Some(StudyWidgets {
            lessons,
            bands,
            panel,
            text,
            controls,
            commands,
            why,
        });
        self.native.world.set_active(panel, false)?;
        self.native.world.set_active(why, false)?;
        Ok(())
    }
    fn lesson_browser(&mut self) -> Result<Vec<ObjectHandle>, WorldError> {
        let panel = self.native.panel([680., 480.])?;
        self.pages.push((Screen::Lessons, panel));
        self.native
            .text(panel, "Lessons", [640., 40.], [0., 205.])?;
        let viewport = self.native.rect(panel, [640., 340.], [0., 0.])?;
        let list = self.native.rect(viewport, [640., 0.], [0., 0.])?;
        self.native.stack(list, UiAxis::Vertical, 4);
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
        let entries: Vec<_> = (0..12)
            .map(|index| ("", MenuAction::Lesson(index)))
            .collect();
        self.list_rows(list, &entries, [640., 44.])?;
        let rows = self.rows[&list].clone();
        self.native.text(
            panel,
            "Up/Down: select  Enter: open  Escape: title",
            [640., 32.],
            [0., -210.],
        )?;
        Ok(rows)
    }
    fn puzzle_browser(&mut self) -> Result<Vec<ObjectHandle>, WorldError> {
        let panel = self.native.panel([680., 400.])?;
        self.pages.push((Screen::Puzzles, panel));
        self.native.text(
            panel,
            "Puzzles: choose a rating band",
            [640., 40.],
            [0., 160.],
        )?;
        let list = self.native.rect(panel, [640., 220.], [0., 0.])?;
        self.native.stack(list, UiAxis::Vertical, 8);
        let entries = [
            None,
            Some("under-1200"),
            Some("1200-1799"),
            Some("1800-plus"),
        ]
        .map(|band| ("", MenuAction::PuzzleBand(band)));
        self.list_rows(list, &entries, [640., 44.])?;
        let rows = self.rows[&list].clone();
        self.native.text(
            panel,
            "Ratings describe puzzles, not your playing strength.\nEscape: title",
            [640., 52.],
            [0., -150.],
        )?;
        Ok(rows)
    }
    pub(in crate::retained) fn synchronize_study(
        &mut self,
        state: Option<StudyState>,
        screen: Screen,
    ) -> Result<(), WorldError> {
        if self.study_state.as_ref() == Some(&(state.clone(), screen)) {
            return Ok(());
        }
        if let Some(state) = &state {
            self.synchronize_why(state.why, screen)?;
            self.synchronize_study_paragraph(state, screen)?;
        }
        let widgets = self.study.as_ref().expect("study widgets");
        if let Some(state) = &state {
            for (&object, (label, enabled)) in widgets.lessons.iter().zip(&state.lessons) {
                self.native
                    .set_text(self.native.text_handle(object), label)?;
                self.native
                    .router
                    .controls
                    .get_mut(&object)
                    .expect("row")
                    .enabled = *enabled;
            }
            for (&object, label) in widgets.bands.iter().zip(&state.bands) {
                self.native
                    .set_text(self.native.text_handle(object), label)?;
            }
            for (&object, enabled) in widgets.controls.iter().zip(state.controls) {
                self.native
                    .router
                    .controls
                    .get_mut(&object)
                    .expect("button")
                    .enabled = enabled;
            }
            self.native.set_text(
                self.native.text_handle(widgets.controls[1]),
                if state.controls[1] {
                    "Next"
                } else {
                    "Next [off]"
                },
            )?;
            let active = matches!(screen, Screen::Lesson | Screen::Puzzle);
            self.native.world.set_active(
                widgets.panel,
                active || (state.why && !state.panel.is_empty()),
            )?;
            self.native.world.set_active(widgets.commands, active)?;
        }
        self.study_state = Some((state, screen));
        self.dirty = true;
        Ok(())
    }
    fn synchronize_study_paragraph(
        &mut self,
        state: &StudyState,
        screen: Screen,
    ) -> Result<(), WorldError> {
        let heading_end = if matches!(screen, Screen::Lesson | Screen::Puzzle) {
            state.panel.find('\n').unwrap_or(state.panel.len())
        } else {
            0
        };
        self.paragraph(
            self.study.as_ref().expect("study widgets").text,
            &state.panel,
            &state.emphasis,
            heading_end,
            super::paragraphs::Flow::Fixed,
        )
    }
    pub(super) fn fit_study_background(
        &mut self,
        layout: &worldrunner::ui::UiLayout,
    ) -> Result<(), WorldError> {
        let panel = self.study.as_ref().expect("study widgets").panel;
        if let Some(rect) = layout.rect(panel) {
            self.native.resize_background(rect)?;
        }
        Ok(())
    }
    fn synchronize_why(&mut self, available: bool, screen: Screen) -> Result<(), WorldError> {
        let why = self.study.as_ref().expect("study widgets").why;
        let active = available && matches!(screen, Screen::InGameMenu | Screen::GameOver);
        if active {
            let parent = self
                .pages
                .iter()
                .find(|(page, _)| *page == screen)
                .expect("menu page")
                .1;
            self.native.world.set_parent(
                why,
                Some(parent),
                worldrunner::simulation::ParentSpace::KeepLocal,
            )?;
        }
        self.native.world.set_active(why, active)?;
        Ok(())
    }
    pub(super) fn study_action(&self, action: worldrunner::ui::UiAction) -> Option<MenuAction> {
        match action {
            worldrunner::ui::UiAction::Activated { object }
                if self.study.as_ref().is_some_and(|w| w.why == object) =>
            {
                Some(MenuAction::Why)
            }
            _ => None,
        }
    }
}

fn study_text(
    native: &mut super::NativeUi,
    panel: ObjectHandle,
) -> Result<ComponentHandle, WorldError> {
    use worldrunner::{simulation::ComponentDesc, ui::RectTransform};
    let object = native.text(panel, "", [728., 124.], [0., 22.])?;
    native.world.set_component(
        native.rect_handle(object),
        ComponentDesc::RectTransform(RectTransform {
            anchor_min: glam::Vec2::new(0., 0.5),
            anchor_max: glam::Vec2::new(1., 0.5),
            size_delta: glam::Vec2::new(-32., 124.),
            anchored_position: glam::Vec2::new(0., 22.),
            ..RectTransform::default()
        }),
    )?;
    let text = native.text_handle(object);
    native.left_text(text, 16.)?;
    Ok(text)
}
