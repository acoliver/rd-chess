//! Game-owned identities and object-only transactions over resident chess assets.
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
    seed: u64,
    scene: RenderScene,
    identities: BTreeMap<String, PieceObjects>,
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
        let identities = position
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
            game,
            seed,
            scene,
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

    pub fn undo(&mut self) -> Result<Vec<SceneObjectEdit>, MoveError> {
        self.game.undo()?;
        Ok(self.synchronize())
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

mod driver;
pub use driver::{BoardDriver, CaptureInbox, TransactionQueue, capture_frame, viewer_options};
