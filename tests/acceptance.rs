use chess3d::{PieceKind, Position, Side, Square, board_albedo, board_mesh, render_scene};
use glam::Vec3;
use worldrunner::scene::gpu::{SceneView, Viewport};

#[test]
fn board_and_lineup_lighting_reach_the_governed_frame_consumer() {
    let position = Position::initial(343);
    for build in [render_scene, chess3d::piece_lineup_scene] {
        let scene = build(&position).expect("authored chess lighting must be supported");
        let view = SceneView::new(
            chess3d::CameraPreset::Hero.pose(),
            *scene.effects().camera(),
            Viewport {
                width: 257,
                height: 193,
            },
        )
        .unwrap();
        let frame = worldrunner::scene::gpu::frame::FrameUniform::new(&view, scene.effects())
            .expect("authored chess lighting must reach the frame consumer");
        assert_eq!(frame.exposure.to_bits(), 1.0_f32.to_bits());
        assert!(frame.sun_irradiance.min_element() > 0.0);
        assert!(frame.ambient_sky.min_element() > 0.0);
    }
}

#[test]
fn persisted_current_squares_and_captures_drive_scene_placement() {
    let directory = tempfile::tempdir().unwrap();
    let save = directory.path().join("edited-position.json");
    let initial = Position::initial(343);
    let initial_scene = render_scene(&initial).unwrap();
    let mut edited = initial.clone();
    edited.pieces[0].square = Square::new(0, 3).unwrap();
    edited.pieces.pop();
    edited.save_atomic(&save).unwrap();
    let loaded = Position::load(&save).unwrap();
    assert_eq!(loaded, edited);
    let scene = render_scene(&loaded).unwrap();
    let objects = scene.geometry().objects();
    assert_eq!(objects.len() + 1, initial_scene.geometry().objects().len());
    for vacated in [[-3500, -3500, 50], [3500, 2500, 50]] {
        assert!(
            !objects
                .iter()
                .any(|o| o.transform().position_millimeters == vacated)
        );
    }
    assert!(objects.iter().any(|o| {
        o.transform().position_millimeters == [-3500, -500, 50]
            && o.mesh().raw() == mesh_id(PieceKind::Rook)
            && o.material().raw() == 1
    }));
}

fn mesh_id(kind: PieceKind) -> u32 {
    match kind {
        PieceKind::Pawn => 2,
        PieceKind::Rook => 3,
        PieceKind::Knight => 4,
        PieceKind::Bishop => 5,
        PieceKind::Queen => 6,
        PieceKind::King => 7,
    }
}

#[test]
fn all_initial_pieces_have_expected_side_kind_square_and_scene_placement() {
    let position = Position::initial(343);
    position.validate().unwrap();
    assert_eq!(position.pieces.len(), 32);
    for side in [Side::Ivory, Side::Onyx] {
        for (kind, count) in [
            (PieceKind::Pawn, 8),
            (PieceKind::Rook, 2),
            (PieceKind::Knight, 2),
            (PieceKind::Bishop, 2),
            (PieceKind::Queen, 1),
            (PieceKind::King, 1),
        ] {
            let pieces: Vec<_> = position
                .pieces
                .iter()
                .filter(|p| p.side == side && p.kind == kind)
                .collect();
            assert_eq!(pieces.len(), count, "{side:?} {kind:?}");
            for piece in pieces {
                let expected_rank = match (side, kind) {
                    (Side::Ivory, PieceKind::Pawn) => 1,
                    (Side::Ivory, _) => 0,
                    (Side::Onyx, PieceKind::Pawn) => 6,
                    (Side::Onyx, _) => 7,
                };
                assert_eq!(piece.square.rank, expected_rank);
                if kind == PieceKind::Queen {
                    assert_eq!(piece.square.file, 3);
                    assert_eq!(piece.square.is_dark(), side == Side::Onyx);
                }
            }
        }
    }
}

#[test]
fn initial_piece_scene_placement_matches_board_square_side_and_kind() {
    let position = Position::initial(343);
    let scene = render_scene(&position).unwrap();
    for piece in &position.pieces {
        let side = piece.side;
        let kind = piece.kind;
        let center = [
            i64::from(piece.square.file) * 1000 - 3500,
            i64::from(piece.square.rank) * 1000 - 3500,
            50,
        ];
        let objects: Vec<_> = scene
            .geometry()
            .objects()
            .iter()
            .filter(|o| o.transform().position_millimeters == center)
            .collect();
        assert_eq!(objects.len(), if kind == PieceKind::Knight { 2 } else { 1 });
        let body = objects[0];
        assert_eq!(
            body.material().raw(),
            if side == Side::Ivory { 1 } else { 2 }
        );
        assert_eq!(body.mesh().raw(), mesh_id(kind));
    }
}

#[test]
fn every_tile_center_uv_parity_and_overhead_projection_agree() {
    let mesh = board_mesh();
    let atlas = board_albedo(343, 512);
    let scene = render_scene(&Position::initial(343)).unwrap();
    let view = SceneView::new(
        chess3d::CameraPreset::Overhead.pose(),
        *scene.effects().camera(),
        Viewport {
            width: 1200,
            height: 1200,
        },
    )
    .unwrap();
    let mut dark_count = 0;
    for rank in 0..8u8 {
        let mut last_x = -1.0;
        for file in 0..8u8 {
            let tile = usize::from(rank * 8 + file);
            let center = mesh.positions[tile * 4..tile * 4 + 4]
                .iter()
                .copied()
                .map(Vec3::from)
                .sum::<Vec3>()
                / 4.0;
            assert_eq!(
                center,
                Vec3::new(
                    f32::from(file) * 1000.0 - 3500.0,
                    f32::from(rank) * 1000.0 - 3500.0,
                    50.0
                )
            );
            let uv = mesh.uvs[tile * 4..tile * 4 + 4]
                .iter()
                .fold(glam::Vec2::ZERO, |a, v| a + glam::Vec2::from(*v))
                / 4.0;
            assert_eq!(
                uv,
                glam::Vec2::new((f32::from(file) + 0.5) / 8.0, (f32::from(rank) + 0.5) / 8.0)
            );
            let index = (usize::from(rank) * 64 + 32) * 512 * 4 + (usize::from(file) * 64 + 32) * 4;
            let color = &atlas[index..index + 4];
            let dark = Square::new(file, rank).unwrap().is_dark();
            assert_eq!(dark, color[0] < 128, "tile {file},{rank}: {color:?}");
            assert_eq!(color[3], 255);
            dark_count += usize::from(dark);
            let clip = view.view_projection_matrix() * (center / 1000.0).extend(1.0);
            let ndc = clip.truncate() / clip.w;
            assert!(clip.w > 0.0 && ndc.x.abs() < 1.0 && ndc.y.abs() < 1.0);
            assert!((0.0..1.0).contains(&ndc.z));
            assert!(ndc.x > last_x, "overhead must not mirror files");
            last_x = ndc.x;
            assert_eq!(ndc.y > 0.0, rank >= 4, "ranks must point up the image");
        }
    }
    assert_eq!(dark_count, 32);
}

#[test]
fn changing_seed_changes_scene_textures_but_not_geometry_or_effects() {
    let a = render_scene(&Position::initial(342)).unwrap();
    let b = render_scene(&Position::initial(342 + (1 << 48))).unwrap();
    assert_eq!(a.geometry(), b.geometry());
    assert_eq!(a.effects(), b.effects());
    assert_ne!(a.resources().textures(), b.resources().textures());
}
