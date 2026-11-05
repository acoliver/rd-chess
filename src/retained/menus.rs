use super::{BoardDriver, ComputerGame};
use crate::{Side, rules::GameStatus};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Screen {
    Title,
    ComputerSetup,
    Slots,
    Settings,
    PgnImport,
    PgnGames,
    #[default]
    Play,
    InGameMenu,
    GameOver,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MenuAction {
    Friend,
    Computer,
    Quit,
    Side(usize),
    Level(u8),
    Start,
    Back,
    Resume,
    Undo,
    Restart,
    Resign,
    Title,
    NewGame,
    ClaimDraw,
    Continue,
    Slots,
    Settings,
    Save(u8),
    Load(u8),
    Markers(bool),
    Overhead(bool),
    ImportPgn,
    ExportPgn,
    PgnEntry(usize),
}

pub(super) const TITLE: &[(&str, MenuAction)] = &[
    ("Play a friend", MenuAction::Friend),
    ("Play the computer", MenuAction::Computer),
    ("Quit", MenuAction::Quit),
];
pub(super) const PAUSE: &[(&str, MenuAction)] = &[
    ("Resume", MenuAction::Resume),
    ("Undo", MenuAction::Undo),
    ("Restart", MenuAction::Restart),
    ("Resign", MenuAction::Resign),
    ("Return to title", MenuAction::Title),
];
pub(super) const OVER: &[(&str, MenuAction)] = &[
    ("New game", MenuAction::NewGame),
    ("Title", MenuAction::Title),
];

pub(super) struct Menus {
    pub screen: Screen,
    pub result: Option<String>,
    pub side: usize,
    pub level: u8,
    pub parent: Screen,
}

impl Default for Menus {
    fn default() -> Self {
        Self {
            screen: Screen::Play,
            result: None,
            side: 0,
            level: 1,
            parent: Screen::Title,
        }
    }
}

impl BoardDriver {
    pub fn screen(&self) -> Screen {
        self.menus.screen
    }
    pub(super) fn ui_actions(
        &mut self,
        actions: &[worldrunner::ui::UiAction],
        activate: bool,
    ) -> Result<(), String> {
        for &action in actions {
            if let worldrunner::ui::UiAction::ChoiceSelected { index, .. } = action {
                self.promote(index)?;
            } else if let Some(command) = self.ui.as_ref().and_then(|ui| ui.action(action))
                && (activate
                    || matches!(
                        command,
                        MenuAction::Side(_)
                            | MenuAction::Level(_)
                            | MenuAction::Markers(_)
                            | MenuAction::Overhead(_)
                    ))
            {
                self.menu_action(command)?;
            }
        }
        Ok(())
    }
    pub fn show_title(&mut self) -> Result<(), String> {
        let storage = self.storage.take();
        let reset = self.restart();
        self.storage = storage;
        reset?;
        self.computer = None;
        self.transition(Screen::Title)
    }
    pub fn claim_available(&self) -> bool {
        self.screen() == Screen::Play && self.draw_claim_reason().is_some() && !self.computer_turn()
    }
    pub(super) fn draw_claim_reason(&self) -> Option<&'static str> {
        if self.menus.result.is_some() {
            return None;
        }
        let GameStatus::Ongoing { claims, .. } = self.board.as_ref()?.game().status() else {
            return None;
        };
        if claims.threefold {
            Some("threefold repetition")
        } else if claims.fifty_move {
            Some("fifty-move rule")
        } else {
            None
        }
    }
    pub(super) fn game_ended(&self) -> bool {
        self.menus.result.is_some()
            || self
                .board
                .as_ref()
                .is_some_and(|board| !matches!(board.game().status(), GameStatus::Ongoing { .. }))
    }
    pub(super) fn finish(&mut self, result: String) -> Result<(), String> {
        self.computer_changed();
        self.menus.result = Some(result);
        self.clear_selection()?;
        self.autosave();
        self.transition(Screen::GameOver)
    }
    pub(super) fn refresh_game_over(&mut self) -> Result<(), String> {
        if matches!(self.screen(), Screen::Play | Screen::InGameMenu) && self.game_ended() {
            self.transition(Screen::GameOver)?;
        }
        Ok(())
    }
    pub(super) fn transition(&mut self, screen: Screen) -> Result<(), String> {
        self.clear_selection()?;
        if !matches!(screen, Screen::PgnImport | Screen::PgnGames) {
            self.pgn.message = None;
        }
        self.menus.screen = screen;
        self.sync_menu_ui()
    }
    pub(super) fn sync_menu_ui(&mut self) -> Result<(), String> {
        let state = super::ui::UiState {
            screen: self.screen(),
            promotion: self.promotion_pending(),
            claim: self.claim_available(),
            undo: self.undo_plies().is_ok(),
            continuation: self.can_continue().then_some(()),
            settings: self.preferences,
            labels: self.slot_labels(),
            setup: (self.menus.side, self.menus.level),
            pgn_entries: self.pgn_entries(),
        };
        if let Some(ui) = &mut self.ui {
            ui.synchronize(state, self.viewport)
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    pub(super) fn menu_action(&mut self, action: MenuAction) -> Result<(), String> {
        match action {
            MenuAction::Friend => {
                self.computer_changed();
                self.computer = None;
                self.restart()?;
                self.transition(Screen::Play)?;
            }
            MenuAction::Computer => self.transition(Screen::ComputerSetup)?,
            MenuAction::Quit => self.exit = true,
            MenuAction::Side(index) if self.screen() != Screen::Settings => self.menus.side = index,
            MenuAction::Level(level) if self.screen() != Screen::Settings => {
                self.menus.level = level;
            }
            MenuAction::Start => self.start_computer_game()?,
            MenuAction::Back if matches!(self.screen(), Screen::PgnImport | Screen::PgnGames) => {
                self.pgn_back()?;
            }
            MenuAction::Back if self.screen() == Screen::Slots => {
                self.transition(self.menus.parent)?;
            }
            MenuAction::Back | MenuAction::Title => self.show_title()?,
            MenuAction::Resume => self.transition(Screen::Play)?,
            MenuAction::Undo => {
                self.undo()?;
                self.transition(Screen::Play)?;
            }
            MenuAction::Restart | MenuAction::NewGame => {
                self.restart()?;
                self.reset_camera();
                self.transition(Screen::Play)?;
            }
            MenuAction::Resign => self.resign()?,
            MenuAction::ClaimDraw => self.claim_draw()?,
            MenuAction::ImportPgn | MenuAction::ExportPgn | MenuAction::PgnEntry(_) => {
                self.pgn_menu_action(action)?;
            }
            action => self.storage_menu_action(action)?,
        }
        Ok(())
    }
    fn start_computer_game(&mut self) -> Result<(), String> {
        self.restart()?;
        let seed = self.board.as_ref().expect("playable board").position().seed;
        let human = match self.menus.side {
            0 => Side::Ivory,
            1 => Side::Onyx,
            _ if super::computer::move_seed(seed, 0) & 1 == 0 => Side::Ivory,
            _ => Side::Onyx,
        };
        let level = self.menus.level;
        self.set_computer(ComputerGame::new(human, level, seed)?)?;
        self.transition(Screen::Play)
    }
    fn resign(&mut self) -> Result<(), String> {
        let side = self.computer_game().map_or_else(
            || {
                self.board
                    .as_ref()
                    .expect("playable board")
                    .game()
                    .side_to_move()
            },
            ComputerGame::human,
        );
        let winner = match side {
            Side::Ivory => Side::Onyx,
            Side::Onyx => Side::Ivory,
        };
        self.finish(format!(
            "{} wins: {} resigned",
            super::status::side_name(winner),
            super::status::side_name(side)
        ))
    }
    fn claim_draw(&mut self) -> Result<(), String> {
        let reason = self.draw_claim_reason().ok_or("no draw claim available")?;
        self.finish(format!("Draw: {reason} claimed"))
    }
}
