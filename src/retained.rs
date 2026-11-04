//! Game-owned identities and object-only transactions over resident chess assets.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]
use crate::{
    Position, SceneBuildError, render_scene,
    rules::{ChessState, MoveError},
};
use std::collections::BTreeMap;
use worldrunner::scene::{
    plan::{RenderScene, SceneObject},
    viewer::{ObjectId, SceneObjectEdit},
};

/// The eye identity is reserved even before a pawn promotes to a knight.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PieceObjects {
    pub body: ObjectId,
    pub eye: ObjectId,
}

pub struct RetainedBoard {
    game: ChessState,
    initial: ChessState,
    seed: u64,
    scene: RenderScene,
    identities: BTreeMap<String, PieceObjects>,
    initial_identities: BTreeMap<String, PieceObjects>,
    objects: BTreeMap<ObjectId, SceneObject>,
}

impl RetainedBoard {
    pub fn new(game: ChessState, seed: u64) -> Result<Self, SceneBuildError> {
        let position = game.to_position(seed);
        let scene = render_scene(&position)?;
        let objects: BTreeMap<_, _> = scene
            .geometry()
            .objects()
            .iter()
            .copied()
            .enumerate()
            .map(|(i, object)| (ObjectId(i as u64), object))
            .collect();
        let mut next = 4;
        let mut reserved = objects.len() as u64;
        let identities: BTreeMap<_, _> = position
            .pieces
            .iter()
            .map(|piece| {
                let body = ObjectId(next);
                next += 1;
                let eye = if piece.kind == crate::PieceKind::Knight {
                    let eye = ObjectId(next);
                    next += 1;
                    eye
                } else {
                    let eye = ObjectId(reserved);
                    reserved += 1;
                    eye
                };
                (piece.id.clone(), PieceObjects { body, eye })
            })
            .collect();
        Ok(Self {
            initial: game.clone(),
            game,
            seed,
            scene,
            initial_identities: identities.clone(),
            identities,
            objects,
        })
    }

    /// The immutable initial asset tables are built once, never on play or undo.
    pub fn scene(&self) -> &RenderScene {
        &self.scene
    }
    pub fn position(&self) -> Position {
        self.game.to_position(self.seed)
    }
    pub fn identities(&self) -> &BTreeMap<String, PieceObjects> {
        &self.identities
    }
    pub fn objects(&self) -> &BTreeMap<ObjectId, SceneObject> {
        &self.objects
    }

    pub fn play_uci(&mut self, uci: &str) -> Result<Vec<SceneObjectEdit>, MoveError> {
        let chess_move = self.game.parse_uci(uci)?;
        self.game.play(chess_move)?;
        Ok(self.synchronize())
    }

    pub fn game(&self) -> &ChessState {
        &self.game
    }

    pub fn play(
        &mut self,
        chess_move: crate::rules::LegalMove,
    ) -> Result<Vec<SceneObjectEdit>, MoveError> {
        self.game.play(chess_move)?;
        Ok(self.synchronize())
    }

    pub fn restart(&mut self) -> Vec<SceneObjectEdit> {
        self.game = self.initial.clone();
        self.identities = self.initial_identities.clone();
        self.synchronize()
    }

    pub fn undo(&mut self) -> Result<Vec<SceneObjectEdit>, MoveError> {
        self.undo_plies(1)
    }

    fn undo_plies(&mut self, count: usize) -> Result<Vec<SceneObjectEdit>, MoveError> {
        for _ in 0..count {
            self.game.undo()?;
        }
        Ok(self.synchronize())
    }

    pub(super) fn set_position(&mut self, game: ChessState) -> Vec<SceneObjectEdit> {
        self.identities = game
            .to_position(self.seed)
            .pieces
            .iter()
            .enumerate()
            .map(|(index, piece)| {
                let body = ObjectId(4 + index as u64 * 2);
                (
                    piece.id.clone(),
                    PieceObjects {
                        body,
                        eye: ObjectId(body.0 + 1),
                    },
                )
            })
            .collect();
        self.synchronize_game(game)
    }

    pub(super) fn synchronize_game(&mut self, game: ChessState) -> Vec<SceneObjectEdit> {
        self.game = game;
        self.synchronize()
    }

    fn synchronize(&mut self) -> Vec<SceneObjectEdit> {
        let mut next: BTreeMap<_, _> = self
            .objects
            .iter()
            .filter(|(id, _)| id.0 < 4)
            .map(|(&id, &object)| (id, object))
            .collect();
        for piece in &self.position().pieces {
            let ids = self.identities[&piece.id];
            for (id, object) in [ids.body, ids.eye]
                .into_iter()
                .zip(crate::scene::piece_objects(piece, true))
            {
                next.insert(id, object);
            }
        }
        let mut edits = Vec::new();
        for &id in self.objects.keys().filter(|id| !next.contains_key(id)) {
            edits.push(SceneObjectEdit::Remove(id));
        }
        for (&id, &object) in &next {
            match self.objects.get(&id) {
                None => edits.push(SceneObjectEdit::Add { id, object }),
                Some(before) if before != &object => {
                    edits.push(SceneObjectEdit::Replace { id, object });
                }
                _ => (),
            }
        }
        self.objects = next;
        edits
    }
}
mod computer;

mod driver;
mod host;
mod interaction;
mod markers;
mod menus;
pub mod persistence;
mod picking;
mod pump;
mod save_menus;
mod saves;
mod status;
mod training;
mod ui;
mod watchdog;
pub use computer::ComputerGame;
pub use driver::{BoardDriver, CaptureInbox, InputInbox, TransactionQueue};
pub use host::{capture_frame, viewer_options};
pub use menus::Screen;
pub use pump::ProgressPump;
pub use training::training_move_pixels;
