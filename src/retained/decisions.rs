use super::{BoardDriver, Screen};
use crate::Side;

impl BoardDriver {
    pub(super) fn escape_menu(&mut self) -> Result<bool, String> {
        let screen = match self.screen() {
            Screen::Play => Screen::InGameMenu,
            Screen::InGameMenu => Screen::Play,
            Screen::ComputerSetup
            | Screen::Settings
            | Screen::ReviewGames
            | Screen::Lessons
            | Screen::Puzzles => {
                self.show_title()?;
                return Ok(true);
            }
            Screen::Lesson | Screen::Puzzle => {
                self.training_action(super::menus::MenuAction::StudyExit)?;
                return Ok(true);
            }
            Screen::Slots => self.menus.parent,
            Screen::Review => self.review.as_ref().expect("session").parent,
            screen => screen,
        };
        self.transition(screen)?;
        Ok(false)
    }
    pub(super) fn offer_draw(&mut self) -> Result<(), String> {
        if self.computer.is_some() {
            self.interaction.announcement = Some("Computer declined the draw offer".into());
            return self.transition(Screen::Play);
        }
        let side = self
            .board
            .as_ref()
            .expect("playable board")
            .game()
            .side_to_move();
        self.menus.offer = Some(side);
        self.interaction.announcement =
            Some(format!("{} offers a draw", super::status::side_name(side)));
        self.transition(Screen::DrawOffer)
    }
    pub(super) fn decline_draw(&mut self) -> Result<(), String> {
        let offerer = self.menus.offer.take().expect("pending draw offer");
        let side = if offerer == Side::Ivory {
            Side::Onyx
        } else {
            Side::Ivory
        };
        self.interaction.announcement = Some(format!(
            "{} declined the draw offer",
            super::status::side_name(side)
        ));
        self.transition(Screen::Play)
    }
}
