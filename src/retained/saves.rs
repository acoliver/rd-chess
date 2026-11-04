use super::{
    BoardDriver, ComputerGame, RetainedBoard, Screen,
    persistence::{GamePayload, GameSettings, SavedMode, Storage},
};
use std::path::PathBuf;
use worldrunner::scene::viewer::{OrbitCamera, RetainedUpdate, SceneObjectEdit};

impl BoardDriver {
    pub fn open_store(&mut self, root: PathBuf) {
        let mut storage = Storage::open(root);
        match storage.settings() {
            Ok(settings) => {
                self.preferences = settings;
                self.menus.level = settings.level;
                self.menus.side = settings.side;
            }
            Err(error) => storage.error = Some(error),
        }
        storage.refresh();
        self.storage = Some(storage);
    }
    pub fn can_continue(&self) -> bool {
        self.storage.as_ref().is_some_and(|s| s.current.is_some())
    }
    pub fn slot_labels(&self) -> [String; 3] {
        self.storage.as_ref().map_or_else(
            || std::array::from_fn(|_| "Empty".into()),
            |s| s.labels.clone(),
        )
    }
    pub fn settings(&self) -> GameSettings {
        self.preferences
    }
    pub fn set_settings(&mut self, settings: GameSettings) -> Result<(), String> {
        settings.validate()?;
        self.preferences = settings;
        self.menus.level = settings.level;
        self.menus.side = settings.side;
        self.apply_camera_setting()?;
        let result = self
            .storage
            .as_ref()
            .ok_or_else(|| "no application store".into())
            .and_then(|s| s.save_settings(settings));
        self.storage_result(result)
    }
    pub(super) fn apply_camera_setting(&mut self) -> Result<(), String> {
        if self.camera.is_some() {
            let board = self.board.as_ref().expect("game settings require a board");
            let preset = if self.preferences.overhead {
                crate::CameraPreset::Overhead
            } else {
                crate::CameraPreset::Hero
            };
            let options = super::viewer_options(board.scene(), preset, self.viewport);
            self.camera = Some(super::driver::CameraControls {
                orbit: OrbitCamera::new(options.pose, options.orbit).map_err(|e| e.to_string())?,
                dirty: true,
            });
        }
        Ok(())
    }
    pub fn saved_payload(&self, slot: u8) -> Result<GamePayload, String> {
        let name = slot_name(slot)?;
        self.storage
            .as_ref()
            .ok_or("no application store")?
            .read(&name)?
            .ok_or_else(|| "save slot is empty".into())
    }
    pub fn save_game(&mut self, slot: u8) -> Result<(), String> {
        let result = slot_name(slot).and_then(|name| self.write_game(&name));
        self.storage_result(result)
    }
    pub fn load_game(&mut self, slot: u8) -> Result<(), String> {
        let result = self
            .saved_payload(slot)
            .and_then(|payload| self.restore_game(payload));
        self.storage_result(result)
    }
    pub fn continue_game(&mut self) -> Result<(), String> {
        let result = self
            .storage
            .as_ref()
            .ok_or("no application store")
            .map_err(str::to_owned)
            .and_then(|s| s.read("current"))
            .and_then(|p| p.ok_or_else(|| "no resumable game".into()))
            .and_then(|payload| {
                if !payload.resumable()? {
                    return Err("game has finished".into());
                }
                self.restore_game(payload)
            });
        self.storage_result(result)
    }
    pub(super) fn autosave(&mut self) {
        if self.storage.is_some() {
            let result = self.write_game("current");
            let _ = self.storage_result(result);
        }
    }
    fn write_game(&mut self, name: &str) -> Result<(), String> {
        if self.screen() == Screen::Slots && self.menus.parent == Screen::Title {
            let storage = self.storage.as_mut().ok_or("no application store")?;
            let payload = storage.read("current")?.ok_or("no current game to save")?;
            storage.write(name, &payload)?;
            storage.refresh();
            return Ok(());
        }
        let board = self
            .board
            .as_ref()
            .ok_or("editor scenes have no legal game")?;
        let mode = self
            .computer_game()
            .map_or(SavedMode::TwoPlayer, |c| SavedMode::Computer {
                human: c.human(),
                level: c.level(),
            });
        let result = self.menus.result.clone().or_else(|| {
            self.game_ended()
                .then(|| super::status::status_text(board.game()))
        });
        let payload = GamePayload::from_game(board.game(), board.position().seed, mode, result);
        let storage = self.storage.as_mut().ok_or("no application store")?;
        storage.write(name, &payload)?;
        storage.refresh();
        Ok(())
    }
    fn storage_result(&mut self, result: Result<(), String>) -> Result<(), String> {
        if let Err(error) = &result
            && let Some(storage) = &mut self.storage
        {
            storage.error = Some(error.clone());
        }
        result
    }
    fn restore_game(&mut self, payload: GamePayload) -> Result<(), String> {
        let game = payload.replay()?;
        let initial =
            crate::rules::ChessState::from_fen(&payload.initial_fen).map_err(|e| e.to_string())?;
        let mut board = RetainedBoard::new(initial, payload.seed).map_err(|e| e.to_string())?;
        board.game = game;
        board.synchronize();
        if let Some(computer) = &mut self.computer {
            computer.changed();
        }
        self.computer = None;
        self.clear_selection()?;
        let mut edits = self.markers.clear();
        if let Some(old) = &self.board {
            edits.extend(
                old.objects()
                    .keys()
                    .filter(|id| id.0 >= 4)
                    .copied()
                    .map(SceneObjectEdit::Remove),
            );
        }
        edits.extend(
            board
                .objects()
                .iter()
                .filter(|(id, _)| id.0 >= 4)
                .map(|(&id, &object)| SceneObjectEdit::Add { id, object }),
        );
        self.resources = Some(RetainedUpdate::Resources {
            resources: board.scene().resources().clone(),
            meshes: board.scene().geometry().meshes().to_vec(),
        });
        self.markers.base = board
            .identities()
            .values()
            .map(|ids| ids.body.0.max(ids.eye.0))
            .max()
            .expect("saved kings")
            + 1;
        self.board = Some(board);
        self.pending.borrow_mut().push_back(edits);
        self.menus.result = payload.result;
        self.menus.screen = Screen::Play;
        if let SavedMode::Computer { human, level } = payload.mode {
            self.set_computer(ComputerGame::new(human, level, payload.seed)?)?;
        }
        self.refresh_game_over()?;
        self.autosave();
        self.sync_menu_ui()
    }
}
fn slot_name(slot: u8) -> Result<String, String> {
    if !(1..=3).contains(&slot) {
        return Err("manual slot must be 1..=3".into());
    }
    Ok(format!("slot-{slot}"))
}
