use super::{GameUi, MenuAction, Screen, UiState};
use worldrunner::{
    simulation::{ComponentDesc, ObjectHandle, WorldError},
    ui::UiAxis,
};

impl GameUi {
    pub(super) fn storage_pages(&mut self) -> Result<(), WorldError> {
        let title = self
            .pages
            .iter()
            .find(|p| p.0 == Screen::Title)
            .expect("title")
            .1;
        self.choice_row(
            title,
            -140.,
            &[
                ("Continue", MenuAction::Continue),
                ("Saved games", MenuAction::Slots),
                ("Settings", MenuAction::Settings),
            ],
        )?;
        let pause = self
            .pages
            .iter()
            .find(|p| p.0 == Screen::InGameMenu)
            .expect("pause")
            .1;
        self.choice_row(pause, -155., &[("Save / Load", MenuAction::Slots)])?;
        self.slots_page()?;
        self.settings_page()
    }
    fn choice_row(
        &mut self,
        panel: ObjectHandle,
        y: f32,
        entries: &[(&str, MenuAction)],
    ) -> Result<(), WorldError> {
        let list = self.native.rect(panel, [580., 36.], [0., y])?;
        self.native.stack(list, UiAxis::Horizontal, 8);
        let width = (580. - f32::from(u16::try_from(entries.len() - 1).expect("small list")) * 8.)
            / f32::from(u16::try_from(entries.len()).expect("small list"));
        self.list_rows(list, entries, [width, 36.])
    }
    fn slots_page(&mut self) -> Result<(), WorldError> {
        let panel = self.native.panel([620., 460.])?;
        self.pages.push((Screen::Slots, panel));
        self.native
            .text(panel, "Saved games", [580., 36.], [0., 200.])?;
        for slot in 1..=3 {
            let y = 140. - f32::from(slot - 1) * 112.;
            let save = format!("Save slot {slot}");
            let load = format!("Load slot {slot}");
            self.choice_row(
                panel,
                y,
                &[
                    (save.as_str(), MenuAction::Save(slot)),
                    (load.as_str(), MenuAction::Load(slot)),
                ],
            )?;
        }
        self.choice_row(panel, -190., &[("Back", MenuAction::Back)])
    }
    fn settings_page(&mut self) -> Result<(), WorldError> {
        let panel = self.native.panel([620., 550.])?;
        self.pages.push((Screen::Settings, panel));
        self.native
            .text(panel, "Settings", [580., 36.], [0., 245.])?;
        self.settings_summary = Some(self.native.text(panel, "", [600., 60.], [0., 185.])?);
        self.native
            .text(panel, "Default computer level", [580., 28.], [0., 125.])?;
        let labels: Vec<_> = (1..=8).map(|level| level.to_string()).collect();
        let entries: Vec<_> = labels
            .iter()
            .zip(1..=8)
            .map(|(label, level)| (label.as_str(), MenuAction::Level(level)))
            .collect();
        self.choice_row(panel, 88., &entries)?;
        self.native
            .text(panel, "Default human side", [580., 28.], [0., 45.])?;
        self.choice_row(
            panel,
            8.,
            &[
                ("White", MenuAction::Side(0)),
                ("Black", MenuAction::Side(1)),
                ("Random", MenuAction::Side(2)),
            ],
        )?;
        self.native
            .text(panel, "Legal-move markers", [580., 28.], [0., -35.])?;
        self.choice_row(
            panel,
            -72.,
            &[
                ("On", MenuAction::Markers(true)),
                ("Off", MenuAction::Markers(false)),
            ],
        )?;
        self.native
            .text(panel, "Camera", [580., 28.], [0., -115.])?;
        self.choice_row(
            panel,
            -152.,
            &[
                ("Hero", MenuAction::Overhead(false)),
                ("Overhead", MenuAction::Overhead(true)),
            ],
        )?;
        self.choice_row(panel, -230., &[("Back", MenuAction::Back)])
    }
    pub(super) fn sync_storage_controls(&mut self, state: &UiState) -> Result<(), WorldError> {
        for (object, action) in self.rows.clone() {
            match action {
                MenuAction::Continue => {
                    self.native
                        .world
                        .set_active(object, state.continuation.is_some())?;
                }
                MenuAction::Save(slot) | MenuAction::Load(slot) => {
                    let verb = if matches!(action, MenuAction::Save(_)) {
                        "Save"
                    } else {
                        "Load"
                    };
                    self.replace_text(
                        object,
                        &format!("{verb} {slot}\n{}", state.labels[usize::from(slot - 1)]),
                    )?;
                }
                _ => (),
            }
        }
        if let Some(object) = self.settings_summary {
            let settings = state.settings;
            self.replace_text(
                object,
                &format!(
                    "Level {} / {}\nMarkers {} / {} camera",
                    settings.level,
                    ["White", "Black", "Random"][settings.side],
                    if settings.markers { "on" } else { "off" },
                    if settings.overhead {
                        "overhead"
                    } else {
                        "hero"
                    }
                ),
            )?;
        }
        if let Some(object) = self.setup_summary {
            self.replace_text(
                object,
                &format!(
                    "Your side: {} / Level {}",
                    ["White", "Black", "Random"][state.setup.0],
                    state.setup.1
                ),
            )?;
        }
        Ok(())
    }
    fn replace_text(&mut self, object: ObjectHandle, text: &str) -> Result<(), WorldError> {
        let handle = self.native.text_handle(object);
        let ComponentDesc::Text(mut run) = self.native.world.component(handle)?.desc.clone() else {
            unreachable!()
        };
        text.clone_into(&mut run.text);
        self.native
            .world
            .set_component(handle, ComponentDesc::Text(run))
    }
}
