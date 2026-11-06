use super::{BoardDriver, ComputerGame};
use crate::{Side, rules::GameStatus};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Screen {
    Title,
    ComputerSetup,
    Lessons,
    Puzzles,
    Lesson,
    Puzzle,
    Slots,
    Settings,
    ReviewGames,
    Review,
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
    Lessons,
    Puzzles,
    Lesson(usize),
    LinkedLesson(usize),
    PuzzleBand(Option<&'static str>),
    StudyNext,
    StudyBack,
    StudyHint,
    StudyRetry,
    StudyExit,
    Why,
    Continue,
    Slots,
    Settings,
    Save(u8),
    Load(u8),
    Markers(bool),
    Overhead(bool),
    Review,
    ReviewGames,
    ReviewSaved(u8),
    ReviewSeek(usize),
    ReviewText(usize),
    ReviewStart,
    ReviewBack,
    ReviewForward,
    ReviewEnd,
    Moment(usize),
    WhatChanged,
    ShowReply,
    TryAgain,
    ReviewReturn,
    ReviewExit,
    RelatedLesson,
    ImportPgn,
    ExportPgn,
    PgnEntry(usize),
}

pub(super) const TITLE: &[(&str, MenuAction)] = &[
    ("Continue", MenuAction::Continue),
    ("Play a friend", MenuAction::Friend),
    ("Play the computer", MenuAction::Computer),
    ("Lessons", MenuAction::Lessons),
    ("Puzzles", MenuAction::Puzzles),
    ("Saved games", MenuAction::Slots),
    ("Review games", MenuAction::ReviewGames),
    ("Settings", MenuAction::Settings),
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
        self.leave_training();
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
        if self.screen() == Screen::Review
            && screen != Screen::Review
            && !(screen == Screen::Lesson && self.review_lesson_active())
        {
            self.leave_review()?;
        }
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
            review: self.review_ui_state(),
        };
        let study = self.study_ui_state();
        let references = self.review_text_links(&state.review);
        if let Some(ui) = &mut self.ui {
            ui.set_review_links(references);
            ui.synchronize_study(study, state.screen)
                .map_err(|e| e.to_string())?;
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
            action => self.activity_menu_action(action)?,
        }
        Ok(())
    }
    fn activity_menu_action(&mut self, action: MenuAction) -> Result<(), String> {
        match action {
            MenuAction::LinkedLesson(index) => self.open_text_lesson(index)?,
            MenuAction::Lessons
            | MenuAction::Puzzles
            | MenuAction::Lesson(_)
            | MenuAction::PuzzleBand(_)
            | MenuAction::StudyNext
            | MenuAction::StudyBack
            | MenuAction::StudyHint
            | MenuAction::StudyRetry
            | MenuAction::StudyExit
            | MenuAction::Why => self.training_action(action)?,
            MenuAction::ImportPgn | MenuAction::ExportPgn | MenuAction::PgnEntry(_) => {
                self.pgn_menu_action(action)?;
            }
            MenuAction::Review
            | MenuAction::ReviewGames
            | MenuAction::ReviewSaved(_)
            | MenuAction::ReviewSeek(_)
            | MenuAction::ReviewText(_)
            | MenuAction::ReviewStart
            | MenuAction::ReviewBack
            | MenuAction::ReviewForward
            | MenuAction::ReviewEnd
            | MenuAction::Moment(_)
            | MenuAction::WhatChanged
            | MenuAction::ShowReply
            | MenuAction::TryAgain
            | MenuAction::ReviewReturn
            | MenuAction::ReviewExit
            | MenuAction::RelatedLesson => self.review_action(action)?,
            action => self.storage_menu_action(action)?,
        }
        Ok(())
    }
    fn leave_training(&mut self) {
        if let Some(training) = &mut self.training
            && let Some(orbit) = training.leave()
        {
            let camera = self.camera.as_mut().expect("configured camera");
            camera.orbit = orbit;
            camera.dirty = true;
        }
        self.persist_training_progress();
        self.reset_camera();
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
