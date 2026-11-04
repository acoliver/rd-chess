use crate::Square;
use std::collections::BTreeSet;
use worldrunner::scene::{
    plan::{MaterialHandle, MeshHandle, SceneObject, SceneRotation, SceneTransform},
    viewer::{ObjectId, SceneObjectEdit},
};

#[derive(Default)]
pub(super) struct Markers {
    pub base: u64,
    pub visible: BTreeSet<Square>,
}
impl Markers {
    pub fn edits(&self, next: &BTreeSet<Square>) -> Vec<SceneObjectEdit> {
        let mut edits: Vec<_> = self
            .visible
            .difference(next)
            .map(|&square| SceneObjectEdit::Remove(self.id(square)))
            .collect();
        edits.extend(
            next.difference(&self.visible)
                .map(|&square| SceneObjectEdit::Add {
                    id: self.id(square),
                    object: marker(square),
                }),
        );
        edits
    }
    pub fn clear(&mut self) -> Vec<SceneObjectEdit> {
        let edits = self.edits(&BTreeSet::new());
        self.visible.clear();
        edits
    }
    fn id(&self, square: Square) -> ObjectId {
        ObjectId(self.base + u64::from(square.rank) * 8 + u64::from(square.file))
    }
}
fn marker(square: Square) -> SceneObject {
    SceneObject::new(
        SceneTransform {
            position_millimeters: [
                i64::from(square.file) * 1000 - 3500,
                i64::from(square.rank) * 1000 - 3500,
                65,
            ],
            rotation: SceneRotation::IDENTITY,
            scale_millionths: 1_000_000,
        },
        MeshHandle::from_raw(11),
        MaterialHandle::from_raw(6),
    )
}
