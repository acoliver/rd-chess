use super::{BoardDriver, Screen};
use worldrunner::{
    scene::viewer::{RetainedInput, RetainedPickHit},
    ui::{Clipboard, ClipboardCommand, UiKey},
};

impl BoardDriver {
    pub(super) fn flush_clipboard(&mut self, clipboard: &mut dyn Clipboard) -> Result<(), String> {
        let Some(command) = self.pgn.clipboard.take() else {
            return Ok(());
        };
        let result = match command {
            ClipboardCommand::Copy => self.pgn_document().and_then(|(text, _)| {
                clipboard
                    .write_text(&text)
                    .map_err(|e| format!("clipboard write failed: {e:?}"))?;
                self.pgn.message = Some("PGN copied".into());
                Ok(())
            }),
            ClipboardCommand::Paste => clipboard
                .read_text()
                .map_err(|e| format!("clipboard read failed: {e:?}"))
                .and_then(|text| self.paste_pgn(&text)),
            ClipboardCommand::Cut => unreachable!("PGN has no Cut action"),
        };
        if let Err(error) = result {
            self.pgn.message = Some(format!("PGN error: {error}"));
        }
        self.sync_menu_ui()
    }
    fn paste_pgn(&mut self, text: &str) -> Result<(), String> {
        let games = crate::rules::pgn::parse(text).map_err(|e| e.to_string())?;
        if games.is_empty() {
            return Err("Clipboard contains no PGN games".into());
        }
        // Parse every game before changing either the live board or the selector.
        self.pgn.games = games;
        self.pgn.paste_parent = Some(self.screen());
        if self.pgn.games.len() == 1 {
            self.choose_pgn_game(0)
        } else {
            self.transition(Screen::PgnGames)
        }
    }
    pub(super) fn input_with_clipboard(
        &mut self,
        input: &RetainedInput,
        hit: Option<RetainedPickHit>,
        command: Option<ClipboardCommand>,
        clipboard: &mut dyn Clipboard,
    ) -> Result<(), String> {
        if let Some(command) = command
            && self.screen() == Screen::SaveLabel
            && !self.unfocused
        {
            let response = self
                .ui
                .as_mut()
                .expect("configured UI")
                .clipboard(command, clipboard, self.viewport)
                .map_err(|e| e.to_string())?;
            self.ui_actions(&response.actions, true)?;
        } else {
            let view = self.view;
            self.handle_input(input, view.as_ref(), hit)?;
        }
        self.flush_clipboard(clipboard)
    }
    pub(super) fn select_all(&mut self, input: &RetainedInput) -> Result<bool, String> {
        let primary = if cfg!(target_os = "macos") {
            "super"
        } else {
            "control"
        };
        match input {
            RetainedInput::Key { key, pressed, .. } if key == primary => {
                self.interaction.primary = *pressed;
            }
            RetainedInput::Focused(false) => self.interaction.primary = false,
            _ => {}
        }
        if self.screen() != Screen::SaveLabel || !self.interaction.primary || self.unfocused {
            return Ok(false);
        }
        if let RetainedInput::Key {
            key,
            pressed,
            repeat,
            ..
        } = input
            && key == "a"
        {
            let response = self
                .ui
                .as_mut()
                .expect("configured UI")
                .key(UiKey::SelectAll, *pressed, *repeat, self.viewport)
                .map_err(|e| e.to_string())?;
            self.ui_actions(&response.actions, true)?;
            return Ok(true);
        }
        Ok(false)
    }
    pub(super) fn text_input_area(&self) -> Result<Option<[f64; 4]>, String> {
        self.ui
            .as_ref()
            .map_or(Ok(None), |ui| ui.text_input_area(self.viewport))
    }
}
