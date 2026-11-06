use super::{GameUi, widgets::NativeUi};
use crate::{Side, Square};
use glam::Vec2;
use worldrunner::{
    scene::gpu::SceneView,
    simulation::{ComponentDesc, ComponentHandle, ObjectHandle, WorldError},
    ui::{
        CanvasGroup, CanvasGroupFlags, CanvasRenderer, RectTransform, UiAccessAction,
        UiAccessRequest, UiKey, UiNodeId, UiSemantics,
    },
};

pub(in crate::retained) struct BoardState {
    pub labels: Vec<String>,
    pub orientation: Side,
    pub visible: bool,
}
pub(super) struct BoardControls {
    group: ObjectHandle,
    cells: Vec<(ObjectHandle, ComponentHandle, ComponentHandle)>,
    orientation: Side,
}
impl BoardControls {
    pub fn new(native: &mut NativeUi) -> Result<Self, WorldError> {
        let group = native.rect(native.root, [0., 0.], [0., 0.])?;
        native.world.set_component(
            native.rect_handle(group),
            ComponentDesc::RectTransform(RectTransform {
                anchor_min: Vec2::ZERO,
                anchor_max: Vec2::ONE,
                pivot: Vec2::ZERO,
                size_delta: Vec2::ZERO,
                ..RectTransform::default()
            }),
        )?;
        native
            .router
            .semantics
            .insert(group, UiSemantics::label("Chess board"));
        // Pointer picking must continue to use the published 3D frame, including silhouettes.
        native.world.add_component(
            group,
            ComponentDesc::CanvasGroup(CanvasGroup {
                flags: CanvasGroupFlags {
                    blocks_raycasts: false,
                    ..CanvasGroupFlags::default()
                },
                ..CanvasGroup::default()
            }),
        )?;
        let mut cells = Vec::new();
        for _ in 0..64 {
            cells.push(super::board_focus::control(native, group)?);
        }
        Ok(Self {
            group,
            cells,
            orientation: Side::Ivory,
        })
    }
    pub fn square(&self, object: ObjectHandle) -> Option<Square> {
        self.cells
            .iter()
            .position(|&(id, _, _)| id == object)
            .map(|index| crate::retained::announcements::oriented_square(index, self.orientation))
    }
}
impl GameUi {
    pub(in crate::retained) fn synchronize_board(
        &mut self,
        state: &BoardState,
        view: &SceneView,
    ) -> Result<(), WorldError> {
        let board = self.board.as_mut().expect("board controls");
        board.orientation = state.orientation;
        self.native.world.set_active(board.group, state.visible)?;
        for (index, &(object, rect, graphic)) in board.cells.iter().enumerate() {
            let label = UiSemantics::label(&state.labels[index]);
            if self.native.router.semantics.get(&object) != Some(&label) {
                self.native.router.semantics.insert(object, label);
                self.dirty = true;
            }
            let square = crate::retained::announcements::oriented_square(index, state.orientation);
            let bounds = projected_rect(view, square)?;
            let desc = ComponentDesc::RectTransform(bounds);
            if self.native.world.component(rect)?.desc != desc {
                self.native.world.set_component(
                    graphic,
                    ComponentDesc::CanvasRenderer(CanvasRenderer {
                        flags: worldrunner::ui::CanvasRendererFlags {
                            cull: self.native.router.focused() != Some(object),
                            ..worldrunner::ui::CanvasRendererFlags::default()
                        },
                        geometry: Some(super::board_focus::border(bounds.size_delta)?),
                        ..CanvasRenderer::default()
                    }),
                )?;
                self.native.world.set_component(rect, desc)?;
                self.dirty = true;
            }
        }
        Ok(())
    }
    pub(super) fn synchronize_board_focus(&mut self) -> Result<(), WorldError> {
        let focused = self.native.router.focused();
        for &(object, _, graphic) in &self.board.as_ref().expect("board controls").cells {
            let ComponentDesc::CanvasRenderer(mut renderer) =
                self.native.world.component(graphic)?.desc.clone()
            else {
                unreachable!("board focus graphic");
            };
            let culled = focused != Some(object);
            if renderer.flags.cull != culled {
                renderer.flags.cull = culled;
                self.native
                    .world
                    .set_component(graphic, ComponentDesc::CanvasRenderer(renderer))?;
                self.dirty = true;
            }
        }
        Ok(())
    }
    pub(in crate::retained) fn board_action(
        &self,
        action: worldrunner::ui::UiAction,
    ) -> Option<Square> {
        let worldrunner::ui::UiAction::Activated { object } = action else {
            return None;
        };
        self.board.as_ref().expect("board controls").square(object)
    }
    pub(in crate::retained) fn focused_square(&self) -> Option<Square> {
        self.native
            .router
            .focused()
            .and_then(|id| self.board.as_ref().expect("board controls").square(id))
    }
    pub(in crate::retained) fn focus_square(
        &mut self,
        square: Square,
        viewport: [u32; 2],
    ) -> Result<(), WorldError> {
        let board = self.board.as_ref().expect("board controls");
        let index = (0..64)
            .find(|&index| {
                crate::retained::announcements::oriented_square(index, board.orientation) == square
            })
            .expect("board square");
        self.accessibility(
            UiAccessRequest {
                target: UiNodeId::Object(board.cells[index].0),
                action: UiAccessAction::Focus,
            },
            viewport,
        )?;
        Ok(())
    }
    pub(super) fn board_tab(
        &mut self,
        layout: &worldrunner::ui::UiLayout,
    ) -> Result<worldrunner::ui::UiResponse, WorldError> {
        let before = self.native.router.focused();
        let response =
            self.native
                .router
                .key(&self.native.world, layout, UiKey::Tab, true, false)?;
        if self.native.router.focused() == before {
            return Ok(response);
        }
        // The board is one Tab stop; arrows and assistive focus still reach all 64 squares.
        for _ in 0..64 {
            if self.focused_square().is_none() {
                break;
            }
            self.native
                .router
                .key(&self.native.world, layout, UiKey::Tab, false, false)?;
            self.native
                .router
                .key(&self.native.world, layout, UiKey::Tab, true, false)?;
            if self.native.router.focused() == before {
                break;
            }
        }
        Ok(response)
    }
    pub(super) fn board_navigation(
        &mut self,
        key: UiKey,
        viewport: [u32; 2],
    ) -> Result<bool, WorldError> {
        if !matches!(key, UiKey::Left | UiKey::Right | UiKey::Up | UiKey::Down) {
            return Ok(false);
        }
        let current = self.focused_square();
        let orientation = self.board.as_ref().expect("board controls").orientation;
        if current.is_none() && self.native.router.focused().is_some() {
            return Ok(false);
        }
        let square = current.unwrap_or(crate::retained::announcements::oriented_square(
            0,
            orientation,
        ));
        let sign = if orientation == Side::Ivory { 1 } else { -1 };
        let (file, rank) = match key {
            UiKey::Left => (-sign, 0),
            UiKey::Right => (sign, 0),
            UiKey::Up => (0, sign),
            UiKey::Down => (0, -sign),
            _ => unreachable!(),
        };
        self.focus_square(
            Square {
                file: square.file.saturating_add_signed(file).min(7),
                rank: square.rank.saturating_add_signed(rank).min(7),
            },
            viewport,
        )?;
        Ok(true)
    }
}
fn projected_rect(view: &SceneView, square: Square) -> Result<RectTransform, WorldError> {
    let mut low = Vec2::splat(f32::INFINITY);
    let mut high = Vec2::splat(f32::NEG_INFINITY);
    for (x, y) in [(0., 0.), (1., 0.), (1., 1.), (0., 1.)] {
        let point = view
            .resolved_view()
            .world_to_screen(glam::Vec3::new(
                f32::from(square.file) - 4. + x,
                f32::from(square.rank) - 4. + y,
                0.05,
            ))
            .map_err(|_| WorldError::InvalidInput)?;
        let height = f32::from(u16::try_from(view.viewport().height).expect("UI viewport limit"));
        let point = Vec2::new(point.x, height - point.y);
        low = low.min(point);
        high = high.max(point);
    }
    Ok(RectTransform {
        anchor_min: Vec2::ZERO,
        anchor_max: Vec2::ZERO,
        pivot: Vec2::ZERO,
        anchored_position: low,
        size_delta: high - low,
    })
}
