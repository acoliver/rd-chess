use super::{
    GameUi,
    menus::{self, MenuAction, Screen},
};
use crate::search::LADDER;
use worldrunner::{
    simulation::{ObjectHandle, WorldError},
    ui::{UiAxis, UiControl, UiScrollView, UiSize},
};

impl GameUi {
    pub(super) fn build_pages(&mut self) -> Result<(), WorldError> {
        self.command_page(Screen::Title, "Chess3D", menus::TITLE)?;
        self.setup_page()?;
        self.command_page(Screen::InGameMenu, "Game menu", menus::PAUSE)?;
        self.command_page(Screen::GameOver, "Game over", menus::OVER)?;
        self.storage_pages()?;
        self.pgn_pages()
    }
    fn command_page(
        &mut self,
        screen: Screen,
        title: &str,
        entries: &[(&str, MenuAction)],
    ) -> Result<(), WorldError> {
        let panel = self
            .native
            .panel([620., if screen == Screen::Title { 528. } else { 464. }])?;
        self.pages.push((screen, panel));
        self.native.text(
            panel,
            title,
            [580., 44.],
            [0., if screen == Screen::Title { 200. } else { 148. }],
        )?;
        let count = f32::from(u16::try_from(entries.len()).expect("small menu"));
        let list = self.native.rect(
            panel,
            [560., count * 52. - 8.],
            [0., if screen == Screen::Title { -32. } else { -10. }],
        )?;
        self.native.stack(list, UiAxis::Vertical, 8);
        self.list_rows(list, entries, [560., 44.])?;
        Ok(())
    }
    pub(super) fn list_rows(
        &mut self,
        list: ObjectHandle,
        entries: &[(&str, MenuAction)],
        size: [f32; 2],
    ) -> Result<(), WorldError> {
        let mut actions = Vec::new();
        let mut rows = Vec::new();
        for (index, &(label, action)) in entries.iter().enumerate() {
            let object = self.native.text(list, label, size, [0., 0.])?;
            self.native
                .control(object, UiControl::list_item(list, index));
            if action == MenuAction::Undo {
                self.undo = Some(object);
            }
            actions.push(action);
            rows.push(object);
        }
        self.bindings.insert(list, actions);
        self.rows.insert(list, rows);
        Ok(())
    }
    fn setup_page(&mut self) -> Result<(), WorldError> {
        let panel = self.native.panel([620., 520.])?;
        self.pages.push((Screen::ComputerSetup, panel));
        self.native
            .text(panel, "Play the computer", [580., 40.], [0., 230.])?;
        self.setup_summary = Some(self.native.text(panel, "", [580., 28.], [0., 190.])?);
        let sides = self.native.rect(panel, [570., 40.], [0., 150.])?;
        self.native.stack(sides, UiAxis::Horizontal, 15);
        self.list_rows(
            sides,
            &[
                ("White", MenuAction::Side(0)),
                ("Black", MenuAction::Side(1)),
                ("Random (seeded)", MenuAction::Side(2)),
            ],
            [180., 40.],
        )?;
        self.native
            .text(panel, "Computer levels", [580., 32.], [0., 108.])?;
        self.level_list(panel)?;
        self.native.text(
            panel,
            "Tab: next field  Arrows: select  Enter: confirm",
            [600., 30.],
            [0., -160.],
        )?;
        let buttons = self.native.rect(panel, [570., 44.], [0., -215.])?;
        self.native.stack(buttons, UiAxis::Horizontal, 30);
        self.list_rows(
            buttons,
            &[("Start", MenuAction::Start), ("Back", MenuAction::Back)],
            [270., 44.],
        )?;
        Ok(())
    }
    fn level_list(&mut self, panel: ObjectHandle) -> Result<(), WorldError> {
        let viewport = self.native.rect(panel, [580., 210.], [0., -15.])?;
        let list = self.native.rect(viewport, [580., 0.], [0., 0.])?;
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
        let labels: Vec<_> = LADDER
            .iter()
            .enumerate()
            .map(|(index, profile)| {
                format!("{}. {}\n{}", index + 1, profile.name, profile.description)
            })
            .collect();
        let entries: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(index, text)| {
                (
                    text.as_str(),
                    MenuAction::Level(u8::try_from(index + 1).expect("eight levels")),
                )
            })
            .collect();
        self.list_rows(list, &entries, [580., 72.])
    }
}
