use super::{BoardDriver, Screen, menus::MenuAction};

impl BoardDriver {
    pub(super) fn storage_menu_action(&mut self, action: MenuAction) -> Result<(), String> {
        match action {
            MenuAction::Continue => {
                let _ = self.continue_game();
            }
            MenuAction::Slots => {
                self.menus.parent = self.screen();
                self.transition(Screen::Slots)?;
            }
            MenuAction::Settings => self.transition(Screen::Settings)?,
            MenuAction::Save(slot) => {
                let _ = self.save_game(slot);
            }
            MenuAction::Load(slot) => {
                let _ = self.load_game(slot);
            }
            MenuAction::Side(_)
            | MenuAction::Level(_)
            | MenuAction::Markers(_)
            | MenuAction::Overhead(_) => {
                let mut settings = self.preferences;
                match action {
                    MenuAction::Side(side) => settings.side = side,
                    MenuAction::Level(level) => settings.level = level,
                    MenuAction::Markers(markers) => settings.markers = markers,
                    MenuAction::Overhead(overhead) => settings.overhead = overhead,
                    _ => unreachable!(),
                }
                let _ = self.set_settings(settings);
            }
            action => self.review_action(action)?,
        }
        self.sync_menu_ui()
    }
}
