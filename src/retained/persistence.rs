//! Chess-owned payloads over the engine's transactional application store.
mod progress;
mod settings;
mod storage;
use crate::{
    Side,
    rules::{ChessState, GameStatus},
};
pub use progress::ProgressError;
use serde::{Deserialize, Serialize};
pub use settings::GameSettings;
pub(super) use storage::Storage;

pub const PAYLOAD_VERSION: u32 = 1;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum SavedMode {
    TwoPlayer,
    Computer { human: Side, level: u8 },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GamePayload {
    pub version: u32,
    pub initial_fen: String,
    pub moves: Vec<String>,
    pub seed: u64,
    pub mode: SavedMode,
    pub result: Option<String>,
}
impl GamePayload {
    pub fn from_game(
        game: &ChessState,
        seed: u64,
        mode: SavedMode,
        result: Option<String>,
    ) -> Self {
        let (initial_fen, moves) = game.record();
        Self {
            version: PAYLOAD_VERSION,
            initial_fen,
            moves,
            seed,
            mode,
            result,
        }
    }
    pub fn replay(&self) -> Result<ChessState, String> {
        if self.version != PAYLOAD_VERSION {
            return Err(format!(
                "unsupported chess payload version {}",
                self.version
            ));
        }
        if let SavedMode::Computer { level, .. } = self.mode
            && !(1..=8).contains(&level)
        {
            return Err("saved computer level must be 1..=8".into());
        }
        if self
            .result
            .as_ref()
            .is_some_and(|text| text.is_empty() || text.len() > 256)
        {
            return Err("invalid saved result".into());
        }
        let mut game = ChessState::from_fen(&self.initial_fen).map_err(|e| e.to_string())?;
        for (ply, uci) in self.moves.iter().enumerate() {
            if !matches!(game.status(), GameStatus::Ongoing { .. }) {
                return Err(format!(
                    "saved move {uci} at ply {} follows game end",
                    ply + 1
                ));
            }
            let chess_move = game
                .parse_uci(uci)
                .map_err(|e| format!("saved move {uci} at ply {}: {e}", ply + 1))?;
            game.play(chess_move).map_err(|e| e.to_string())?;
        }
        Ok(game)
    }
    pub(super) fn resumable(&self) -> Result<bool, String> {
        Ok(self.result.is_none() && matches!(self.replay()?.status(), GameStatus::Ongoing { .. }))
    }
    pub(super) fn label(&self) -> String {
        let opponent = match self.mode {
            SavedMode::TwoPlayer => "Two players".into(),
            SavedMode::Computer { level, .. } => format!("vs Level {level}"),
        };
        format!("{opponent}, move {}", self.moves.len().div_ceil(2))
    }
}
