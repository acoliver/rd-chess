//! Procedural chess art and position documents. Geometry is authored in Z-up millimeters.
#![expect(
    clippy::cast_precision_loss,
    reason = "procedural coordinates and texture pixels are deliberately narrowed to GPU f32"
)]
#![expect(
    clippy::cast_possible_truncation,
    reason = "generated mesh buffers are bounded to authored sub-million vertex counts"
)]
#![expect(
    clippy::cast_sign_loss,
    reason = "color channels are clamped to their unsigned byte ranges before conversion"
)]
#![expect(
    clippy::too_many_lines,
    reason = "initial position and scene assembly keep their construction invariants together"
)]
#![expect(
    clippy::too_many_arguments,
    reason = "tile construction explicitly carries position, extent, elevation, and atlas bounds"
)]
#![expect(
    clippy::many_single_char_names,
    reason = "tile geometry uses conventional compact coordinate and UV names"
)]
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::path::Path;
use thiserror::Error;

pub const SAVE_VERSION: u32 = 1;
pub const BOARD_WIDTH_MM: f32 = 8_000.0;
pub const SQUARE_MM: f32 = 1_000.0;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Side {
    Ivory,
    Onyx,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PieceKind {
    Pawn,
    Rook,
    Knight,
    Bishop,
    Queen,
    King,
}
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Square {
    pub file: u8,
    pub rank: u8,
}
impl Square {
    pub fn new(file: u8, rank: u8) -> Result<Self, PositionError> {
        if file < 8 && rank < 8 {
            Ok(Self { file, rank })
        } else {
            Err(PositionError::InvalidSquare(file, rank))
        }
    }
    /// a1 is the dark square at the near-left corner when viewed from white's side.
    pub const fn is_dark(self) -> bool {
        (self.file ^ self.rank).is_multiple_of(2)
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Piece {
    pub id: String,
    pub side: Side,
    pub kind: PieceKind,
    pub square: Square,
}
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Position {
    pub format: String,
    pub version: u32,
    pub seed: u64,
    pub style: String,
    pub pieces: Vec<Piece>,
}
impl Position {
    pub fn initial(seed: u64) -> Self {
        let mut pieces = Vec::with_capacity(32);
        for (side, rank, pawn_rank, back, pawns) in [
            (
                Side::Ivory,
                0,
                1,
                [
                    PieceKind::Rook,
                    PieceKind::Knight,
                    PieceKind::Bishop,
                    PieceKind::Queen,
                    PieceKind::King,
                    PieceKind::Bishop,
                    PieceKind::Knight,
                    PieceKind::Rook,
                ],
                1,
            ),
            (
                Side::Onyx,
                7,
                6,
                [
                    PieceKind::Rook,
                    PieceKind::Knight,
                    PieceKind::Bishop,
                    PieceKind::Queen,
                    PieceKind::King,
                    PieceKind::Bishop,
                    PieceKind::Knight,
                    PieceKind::Rook,
                ],
                6,
            ),
        ] {
            for file in 0..8 {
                pieces.push(Piece {
                    id: format!("{}-{}{}", side_name(side), (b'a' + file) as char, rank + 1),
                    side,
                    kind: back[file as usize],
                    square: Square { file, rank },
                });
            }
            for file in 0..8 {
                pieces.push(Piece {
                    id: format!(
                        "{}-p{}{}",
                        side_name(side),
                        (b'a' + file) as char,
                        pawns + 1
                    ),
                    side,
                    kind: PieceKind::Pawn,
                    square: Square {
                        file,
                        rank: pawn_rank,
                    },
                });
            }
        }
        let value = Self {
            format: "chess3d-position".into(),
            version: SAVE_VERSION,
            seed,
            style: "nocturne-ivory".into(),
            pieces,
        };
        value.validate().expect("built-in initial layout is valid");
        value
    }
    pub fn validate(&self) -> Result<(), PositionError> {
        if self.format != "chess3d-position" || self.version != SAVE_VERSION {
            return Err(PositionError::UnsupportedFormat);
        }
        if self.style != "nocturne-ivory" {
            return Err(PositionError::UnknownStyle(self.style.clone()));
        }
        let mut ids = BTreeSet::new();
        let mut squares = BTreeSet::new();
        if self.pieces.len() > 32 {
            return Err(PositionError::TooManyPieces(self.pieces.len()));
        }
        for p in &self.pieces {
            if p.id.is_empty()
                || p.id.len() > 64
                || !p
                    .id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"-_".contains(&b))
            {
                return Err(PositionError::InvalidId(p.id.clone()));
            }
            if p.square.file >= 8 || p.square.rank >= 8 {
                return Err(PositionError::InvalidSquare(p.square.file, p.square.rank));
            }
            if !ids.insert(&p.id) {
                return Err(PositionError::DuplicateId(p.id.clone()));
            }
            if !squares.insert(p.square) {
                return Err(PositionError::DuplicateSquare(p.square));
            }
        }
        Ok(())
    }
    pub fn to_json(&self) -> Result<String, PositionError> {
        self.validate()?;
        serde_json::to_string_pretty(self).map_err(PositionError::Json)
    }
    pub fn from_json(data: &str) -> Result<Self, PositionError> {
        if data.len() > 1_048_576 {
            return Err(PositionError::DocumentTooLarge);
        }
        let position: Self = serde_json::from_str(data).map_err(PositionError::Json)?;
        position.validate()?;
        Ok(position)
    }
    pub fn save_atomic(&self, path: &Path) -> Result<(), PositionError> {
        let data = self.to_json()?;
        if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent)?;
        }
        let mut writer = atomic_write_file::AtomicWriteFile::open(path)?;
        writer.write_all(data.as_bytes())?;
        writer.write_all(b"\n")?;
        writer.sync_all()?;
        writer.commit()?;
        Ok(())
    }
    pub fn load(path: &Path) -> Result<Self, PositionError> {
        let file = std::fs::File::open(path)?;
        if file.metadata()?.len() > 1_048_576 {
            return Err(PositionError::DocumentTooLarge);
        }
        let mut data = String::new();
        file.take(1_048_577).read_to_string(&mut data)?;
        Self::from_json(&data)
    }
}
fn side_name(side: Side) -> &'static str {
    match side {
        Side::Ivory => "ivory",
        Side::Onyx => "onyx",
    }
}

#[derive(Debug, Error)]
pub enum PositionError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("unsupported position format/version")]
    UnsupportedFormat,
    #[error("unknown marble style '{0}'")]
    UnknownStyle(String),
    #[error("position document exceeds 1 MiB")]
    DocumentTooLarge,
    #[error("invalid square ({0},{1})")]
    InvalidSquare(u8, u8),
    #[error("duplicate square {0:?}")]
    DuplicateSquare(Square),
    #[error("duplicate piece id '{0}'")]
    DuplicateId(String),
    #[error("invalid piece id '{0}'")]
    InvalidId(String),
    #[error("at most 32 pieces are supported, got {0}")]
    TooManyPieces(usize),
}

#[derive(Debug, Error)]
pub enum SceneBuildError {
    #[error(transparent)]
    Position(#[from] PositionError),
    #[error(transparent)]
    Scene(#[from] worldrunner::scene::plan::SceneValidationError),
}

mod geometry;
mod presentation;
mod scene;
mod textures;
pub use geometry::{Mesh, board_mesh, piece_meshes};
pub use presentation::CameraPreset;
pub use scene::{piece_lineup_scene, render_scene};
pub use textures::{board_albedo, marble_albedo};

#[cfg(test)]
mod regressions;
#[cfg(test)]
mod tests;
