use super::*;
use crate::geometry::{board_frame_mesh, board_inlay_mesh, knight_eye_mesh, studio_floor_mesh};

#[test]
fn high_seed_bits_change_the_marble_art() {
    assert!(
        marble_albedo(1, 64, [225, 215, 190])
            .iter()
            .zip(marble_albedo(1 + (1 << 48), 64, [225, 215, 190]))
            .any(|(a, b)| *a != b)
    );
    assert!(
        board_albedo(1, 256)
            .iter()
            .zip(board_albedo(1 + (1 << 48), 256))
            .any(|(a, b)| *a != b)
    );
}

#[test]
fn invalid_save_preserves_existing_document_and_empty_positions_are_supported() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("position.json");
    let mut position = Position::initial(5);
    position.save_atomic(&path).unwrap();
    let original = std::fs::read(&path).unwrap();
    position.pieces[0].square.file = 8;
    assert!(position.save_atomic(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), original);
    position.pieces.clear();
    position.save_atomic(&path).unwrap();
    assert_eq!(Position::load(&path).unwrap(), position);
    assert!(render_scene(&position).is_ok());
    position.style = "unsupported".into();
    assert!(matches!(
        position.validate(),
        Err(PositionError::UnknownStyle(_))
    ));
}

#[test]
fn oversized_and_non_utf8_files_are_rejected() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("position.json");
    std::fs::write(&path, vec![b' '; 1_048_577]).unwrap();
    assert!(matches!(
        Position::load(&path),
        Err(PositionError::DocumentTooLarge)
    ));
    std::fs::write(&path, [255, 254]).unwrap();
    assert!(matches!(Position::load(&path), Err(PositionError::Io(_))));
}

#[test]
fn initial_position_has_standard_orientation_and_piece_counts() {
    let position = Position::initial(77);
    assert_eq!(position, Position::initial(77));
    assert_eq!(position.pieces.len(), 32);
    assert!(Square { file: 0, rank: 0 }.is_dark());
    assert_eq!(
        position
            .pieces
            .iter()
            .find(|piece| piece.side == Side::Ivory && piece.kind == PieceKind::Queen)
            .unwrap()
            .square,
        Square { file: 3, rank: 0 }
    );
    for kind in [
        PieceKind::Pawn,
        PieceKind::Rook,
        PieceKind::Knight,
        PieceKind::Bishop,
        PieceKind::Queen,
        PieceKind::King,
    ] {
        assert_eq!(
            position
                .pieces
                .iter()
                .filter(|piece| piece.kind == kind)
                .count(),
            if kind == PieceKind::Pawn {
                16
            } else if kind == PieceKind::King || kind == PieceKind::Queen {
                2
            } else {
                4
            }
        );
    }
}

#[test]
fn edited_positions_round_trip_and_duplicate_squares_are_rejected() {
    let mut position = Position::initial(1);
    position.pieces[0].square = Square { file: 0, rank: 2 };
    assert_eq!(
        Position::from_json(&position.to_json().unwrap()).unwrap(),
        position
    );
    position.pieces[1].square = position.pieces[0].square;
    assert!(matches!(
        position.validate(),
        Err(PositionError::DuplicateSquare(_))
    ));
    assert!(matches!(
        render_scene(&position),
        Err(SceneBuildError::Position(PositionError::DuplicateSquare(_)))
    ));
}

#[test]
fn version_schema_and_board_invariants_are_rejected() {
    for json in [
        r#"{"format":"chess3d-position","version":99,"seed":1,"style":"nocturne-ivory","pieces":[]}"#,
        r#"{"format":"chess3d-position","version":1,"seed":1,"style":"nocturne-ivory","pieces":[{"id":"!","side":"ivory","kind":"king","square":{"file":1,"rank":1}}]}"#,
        r#"{"format":"chess3d-position","version":1,"seed":1,"style":"nocturne-ivory","pieces":[],"extra":true}"#,
    ] {
        assert!(Position::from_json(json).is_err());
    }
    let mut position = Position::initial(3);
    position.pieces[1].id = position.pieces[0].id.clone();
    assert!(matches!(
        position.validate(),
        Err(PositionError::DuplicateId(_))
    ));
    position.pieces[1].id = "unique".into();
    position.pieces[1].square = Square { file: 8, rank: 0 };
    assert!(matches!(
        position.validate(),
        Err(PositionError::InvalidSquare(8, 0))
    ));
    position.pieces.truncate(32);
    position.pieces.push(position.pieces[0].clone());
    assert!(matches!(
        position.validate(),
        Err(PositionError::TooManyPieces(33))
    ));
    assert!(matches!(
        Position::from_json(&" ".repeat(1_048_577)),
        Err(PositionError::DocumentTooLarge)
    ));
}

#[test]
fn procedural_meshes_are_indexed_finite_and_bounded() {
    for mesh in piece_meshes().into_iter().chain([
        board_mesh(),
        studio_floor_mesh(),
        board_frame_mesh(),
        board_inlay_mesh(),
        knight_eye_mesh(),
    ]) {
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.positions.len(), mesh.normals.len());
        assert_eq!(mesh.positions.len(), mesh.uvs.len());
        assert!(
            mesh.indices
                .iter()
                .all(|index| (*index as usize) < mesh.positions.len())
        );
        assert!(
            mesh.positions
                .iter()
                .flatten()
                .all(|value| value.is_finite())
        );
        assert!(mesh.normals.iter().flatten().all(|value| value.is_finite()));
    }
}

#[test]
fn texture_generation_is_seeded_and_hard_bounded() {
    assert_eq!(
        marble_albedo(9, 32, [210, 210, 200]),
        marble_albedo(9, 32, [210, 210, 200])
    );
    assert_eq!(board_albedo(342, 512), board_albedo(342, 512));
    assert_eq!(board_albedo(342, 4096).len(), 512 * 512 * 4);
    assert_eq!(marble_albedo(9, 4096, [0, 0, 0]).len(), 512 * 512 * 4);
}

#[test]
fn initial_and_study_documents_are_valid() {
    let initial = Position::from_json(include_str!("../assets/positions/initial.json")).unwrap();
    assert_eq!(initial, Position::initial(342));
    let study = Position::from_json(include_str!("../assets/positions/study.json")).unwrap();
    assert_eq!(study.pieces.len(), 32);
    assert_ne!(study, initial);
}

#[test]
fn facade_scene_has_board_six_meshes_and_current_piece_placements() {
    let mut position = Position::initial(12);
    position.pieces[0].square = Square { file: 0, rank: 2 };
    let scene = render_scene(&position).unwrap();
    assert_eq!(scene.geometry().meshes().len(), 11);
    assert_eq!(scene.geometry().objects().len(), 4 + 32 + 4);
    assert_eq!(
        scene.geometry().objects()[4]
            .transform()
            .position_millimeters,
        [-3500, -1500, 50]
    );
    assert_eq!(
        piece_lineup_scene(&position)
            .unwrap()
            .geometry()
            .objects()
            .len(),
        37
    );
    assert_eq!(scene.resources().textures().len(), 7);
}

#[test]
fn atomic_save_load_preserves_an_arbitrary_live_position() {
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("position.json");
    let mut position = Position::initial(42);
    position.pieces[0].square = Square { file: 0, rank: 3 };
    position.pieces.pop();
    position.save_atomic(&path).unwrap();
    assert_eq!(Position::load(&path).unwrap(), position);
    std::fs::remove_file(path).unwrap();
}
