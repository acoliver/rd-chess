use super::BoardDriver;
use crate::Square;
use worldrunner::scene::{
    gpu::SceneView,
    viewer::{MouseButton, ObjectId, RetainedInput, RetainedPickHit},
};

impl BoardDriver {
    pub(super) fn input_hit(
        &self,
        input: &RetainedInput,
        pick: impl FnOnce([f64; 2], &dyn Fn(ObjectId) -> bool) -> Option<RetainedPickHit>,
    ) -> Option<RetainedPickHit> {
        if !matches!(
            input,
            RetainedInput::Button {
                button: MouseButton::Left,
                pressed: false
            }
        ) {
            return None;
        }
        pick(self.interaction.cursor?, &|id| self.pickable(id))
    }

    fn pickable(&self, id: ObjectId) -> bool {
        self.board.as_ref().is_some_and(|board| {
            id == ObjectId(0)
                || board
                    .identities()
                    .values()
                    .any(|ids| ids.body == id || ids.eye == id)
                || self.markers.contains(id)
        })
    }

    pub(super) fn clicked_square(
        &self,
        view: &SceneView,
        point: [f64; 2],
        hit: Option<RetainedPickHit>,
    ) -> Option<Square> {
        if let Some(picked) = hit {
            let board = self.board.as_ref()?;
            let piece = board.position().pieces.into_iter().find(|piece| {
                let ids = board.identities()[&piece.id];
                ids.body == picked.hit.object || ids.eye == picked.hit.object
            });
            return piece.map_or_else(
                || square_at(picked.hit.point_meters),
                |piece| Some(piece.square),
            );
        }
        plane_square(view, point)
    }
}

fn square_at(point: glam::Vec3) -> Option<Square> {
    if !(-4. ..4.).contains(&point.x) || !(-4. ..4.).contains(&point.y) {
        return None;
    }
    Square::new((point.x + 4.).floor() as u8, (point.y + 4.).floor() as u8).ok()
}
fn plane_square(view: &SceneView, point: [f64; 2]) -> Option<Square> {
    let viewport = view.viewport();
    if !point.iter().all(|value| value.is_finite())
        || point[0] < 0.
        || point[1] < 0.
        || point[0] >= f64::from(viewport.width)
        || point[1] >= f64::from(viewport.height)
    {
        return None;
    }
    let ray = view
        .resolved_view()
        .screen_ray(glam::DVec2::from_array(point).as_vec2())
        .ok()?;
    square_at(
        ray.intersect_plane(glam::Vec3::new(0., 0., 0.05), glam::Vec3::Z)
            .ok()?,
    )
}
