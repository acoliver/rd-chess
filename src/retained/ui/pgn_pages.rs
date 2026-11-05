use super::{GameUi, MenuAction, Screen, UiState};
use worldrunner::{
    simulation::{ObjectHandle, WorldError},
    ui::{UiAxis, UiScrollView, UiSize},
};

#[derive(Default)]
pub(super) struct PgnControls {
    lists: Vec<(Screen, ObjectHandle)>,
    rows: Vec<ObjectHandle>,
}

impl GameUi {
    pub(super) fn pgn_pages(&mut self) -> Result<(), WorldError> {
        for (screen, title) in [
            (Screen::PgnImport, "Import PGN: choose a file"),
            (Screen::PgnGames, "Import PGN: choose a game"),
        ] {
            let panel = self.native.panel([620., 380.])?;
            self.pages.push((screen, panel));
            self.native.text(panel, title, [580., 40.], [0., 158.])?;
            let viewport = self.native.rect(panel, [580., 230.], [0., 10.])?;
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
            self.pgn.lists.push((screen, list));
            self.native.text(
                panel,
                "Main line only; comments and variations ignored.",
                [600., 28.],
                [0., -130.],
            )?;
            self.native.text(
                panel,
                "Arrows: select  Enter: open  Escape: back",
                [580., 28.],
                [0., -166.],
            )?;
        }
        Ok(())
    }
    pub(super) fn sync_pgn_controls(&mut self, state: &UiState) -> Result<(), WorldError> {
        if !matches!(state.screen, Screen::PgnImport | Screen::PgnGames) {
            return Ok(());
        }
        if self
            .state
            .as_ref()
            .is_some_and(|old| old.screen == state.screen && old.pgn_entries == state.pgn_entries)
        {
            return Ok(());
        }
        for object in self.pgn.rows.drain(..) {
            self.native.router.controls.remove(&object);
            self.native.router.styles.remove(&object);
            self.rows.retain(|(row, _)| *row != object);
            self.native.world.destroy_object(object)?;
        }
        let list = self
            .pgn
            .lists
            .iter()
            .find(|(screen, _)| *screen == state.screen)
            .expect("PGN screen list")
            .1;
        let mut entries: Vec<_> = state
            .pgn_entries
            .iter()
            .enumerate()
            .map(|(index, text)| (text.as_str(), MenuAction::PgnEntry(index)))
            .collect();
        entries.push(("Back", MenuAction::Back));
        let before = self.rows.len();
        self.list_rows(list, &entries, [580., 44.])?;
        self.pgn
            .rows
            .extend(self.rows[before..].iter().map(|(object, _)| *object));
        Ok(())
    }
}
