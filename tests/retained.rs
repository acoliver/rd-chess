#![cfg(feature = "viewer")]
use chess3d::{
    CameraPreset, Square,
    retained::{BoardDriver, RetainedBoard, viewer_options},
    rules::ChessState,
};
use std::{collections::BTreeMap, time::Duration};
use worldrunner::scene::{
    plan::SceneObject,
    viewer::{
        ObjectId, RetainedDriver, RetainedHost, RetainedTick, RetainedUpdate, SceneObjectEdit,
    },
};

fn id_at(board: &RetainedBoard, square: &str) -> ObjectId {
    let location = square_at(square);
    let piece = board
        .position()
        .pieces
        .into_iter()
        .find(|p| p.square == location)
        .unwrap();
    board.identities()[&piece.id].body
}

fn square_at(square: &str) -> Square {
    let bytes = square.as_bytes();
    Square::new(bytes[0] - b'a', bytes[1] - b'1').unwrap()
}

fn replace(board: &RetainedBoard, from: &str, to: &str, mesh: u32) -> (ObjectId, SceneObject) {
    let id = id_at(board, from);
    let before = board.objects()[&id];
    let square = square_at(to);
    let mut transform = *before.transform();
    transform.position_millimeters = [
        i64::from(square.file) * 1000 - 3500,
        i64::from(square.rank) * 1000 - 3500,
        50,
    ];
    (
        id,
        SceneObject::new(
            transform,
            worldrunner::scene::plan::MeshHandle::from_raw(mesh),
            before.material(),
        ),
    )
}

fn assert_edits(
    actual: &[SceneObjectEdit],
    replacements: &[(ObjectId, SceneObject)],
    removals: &[ObjectId],
    additions: &[(ObjectId, SceneObject)],
) {
    let mut replaced = BTreeMap::new();
    let mut removed = Vec::new();
    let mut added = BTreeMap::new();
    for edit in actual {
        match *edit {
            SceneObjectEdit::Replace { id, object } => {
                assert!(replaced.insert(id, object).is_none());
            }
            SceneObjectEdit::Remove(id) => removed.push(id),
            SceneObjectEdit::Add { id, object } => {
                assert!(added.insert(id, object).is_none());
            }
        }
    }
    assert_eq!(replaced, replacements.iter().copied().collect());
    assert_eq!(removed, removals);
    assert_eq!(added, additions.iter().copied().collect());
}

fn board(fen: &str) -> RetainedBoard {
    RetainedBoard::new(ChessState::from_fen(fen).unwrap(), 342).unwrap()
}

#[test]
fn ordinary_move_replaces_only_its_body() {
    let mut board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let expected = replace(&board, "e2", "e4", 2);
    assert_edits(&board.play_uci("e2e4").unwrap(), &[expected], &[], &[]);
}

#[test]
fn castles_replace_the_king_and_correct_rook() {
    for (uci, king_to, rook_from, rook_to) in
        [("e1g1", "g1", "h1", "f1"), ("e1c1", "c1", "a1", "d1")]
    {
        let mut board = board("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1");
        let expected = [
            replace(&board, "e1", king_to, 7),
            replace(&board, rook_from, rook_to, 3),
        ];
        assert_edits(&board.play_uci(uci).unwrap(), &expected, &[], &[]);
    }
}

#[test]
fn captures_remove_the_victim_including_en_passant() {
    for (fen, uci, from, to, victim) in [
        (
            "4k3/8/8/8/8/3p4/4P3/4K3 w - - 0 1",
            "e2d3",
            "e2",
            "d3",
            "d3",
        ),
        (
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
            "e5d6",
            "e5",
            "d6",
            "d5",
        ),
    ] {
        let mut board = board(fen);
        let expected = replace(&board, from, to, 2);
        let removed = id_at(&board, victim);
        assert_edits(&board.play_uci(uci).unwrap(), &[expected], &[removed], &[]);
    }
}

#[test]
fn promotion_reuses_resident_mesh_material_and_reserved_eye() {
    for (uci, mesh) in [("a7a8q", 6), ("a7a8r", 3), ("a7a8b", 5), ("a7a8n", 4)] {
        let mut board = board("4k3/P7/8/8/8/8/8/4K3 w - - 0 1");
        let expected = replace(&board, "a7", "a8", mesh);
        let eye = board
            .identities()
            .values()
            .find(|ids| ids.body == expected.0)
            .unwrap()
            .eye;
        let additions = if mesh == 4 {
            vec![(
                eye,
                SceneObject::new(
                    *expected.1.transform(),
                    worldrunner::scene::plan::MeshHandle::from_raw(9),
                    worldrunner::scene::plan::MaterialHandle::from_raw(4),
                ),
            )]
        } else {
            vec![]
        };
        assert_edits(&board.play_uci(uci).unwrap(), &[expected], &[], &additions);
    }
}

#[test]
fn knight_eye_moves_and_is_removed_with_body() {
    let mut board = board("4k3/8/8/8/8/2p5/8/1N2K3 w - - 0 1");
    let body = replace(&board, "b1", "d2", 4);
    let eye_id = board
        .identities()
        .values()
        .find(|ids| ids.body == body.0)
        .unwrap()
        .eye;
    let eye = board.objects()[&eye_id];
    let expected_eye = (
        eye_id,
        SceneObject::new(*body.1.transform(), eye.mesh(), eye.material()),
    );
    assert_edits(
        &board.play_uci("b1d2").unwrap(),
        &[body, expected_eye],
        &[],
        &[],
    );
    let pawn = replace(&board, "c3", "d2", 2);
    let mut removed = vec![body.0, eye_id];
    removed.sort();
    assert_edits(&board.play_uci("c3d2").unwrap(), &[pawn], &removed, &[]);
}

#[test]
fn undo_restores_ids_transforms_and_assets_without_rebuilding() {
    for (fen, sequence) in [
        (chess3d::rules::INITIAL_FEN, "e2e4 d7d5 e4d5 g8f6"),
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", "e1g1"),
        ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2", "e5d6"),
        ("4k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7a8n"),
        ("4k3/8/8/8/8/2p5/8/1N2K3 w - - 0 1", "b1d2 c3d2"),
    ] {
        assert_undo(fen, sequence);
    }
}

fn assert_undo(fen: &str, sequence: &str) {
    let mut board = board(fen);
    let original = board.objects().clone();
    let ids = board.identities().clone();
    let position = board.position();
    let meshes = board.scene().geometry().meshes().as_ptr();
    let textures = board.scene().resources().textures().as_ptr();
    let mut host = RetainedHost::new(
        board.scene().clone(),
        viewer_options(board.scene(), CameraPreset::Hero, [64, 64]),
    )
    .unwrap();
    for uci in sequence.split_whitespace() {
        host.apply(RetainedUpdate::Objects(board.play_uci(uci).unwrap()))
            .unwrap();
    }
    for _ in sequence.split_whitespace() {
        host.apply(RetainedUpdate::Objects(board.undo().unwrap()))
            .unwrap();
    }
    assert_eq!(board.position(), position);
    assert_eq!(board.objects(), &original);
    assert_eq!(host.objects().collect::<BTreeMap<_, _>>(), original);
    assert_eq!(board.identities(), &ids);
    assert_eq!(board.scene().geometry().meshes().as_ptr(), meshes);
    assert_eq!(board.scene().resources().textures().as_ptr(), textures);
}

#[test]
fn pressure_keeps_the_exact_pending_transaction_and_other_errors_fail() {
    let mut board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let options = viewer_options(board.scene(), CameraPreset::Hero, [64, 64]);
    let mut host = RetainedHost::with_preparation_limit(board.scene().clone(), options, 1).unwrap();
    let mut driver = BoardDriver::default();
    driver
        .queue()
        .borrow_mut()
        .push_back(board.play_uci("e2e4").unwrap());
    driver
        .queue()
        .borrow_mut()
        .push_back(board.play_uci("e7e5").unwrap());
    let tick = RetainedTick {
        frame: 0,
        elapsed: Duration::ZERO,
        dt: Duration::ZERO,
    };
    driver.update(&mut host, &tick).unwrap();
    assert_eq!(host.accepted_generation(), 1);
    assert_eq!(driver.queue().borrow().len(), 1);
    let objects: Vec<_> = host.objects().collect();
    driver.update(&mut host, &tick).unwrap();
    assert_eq!(driver.queue().borrow().len(), 1);
    assert_eq!(host.objects().collect::<Vec<_>>(), objects);
    let mut host = RetainedHost::new(board.scene().clone(), host.options().clone()).unwrap();
    driver.queue().borrow_mut().clear();
    driver
        .queue()
        .borrow_mut()
        .push_back(vec![SceneObjectEdit::Remove(ObjectId(u64::MAX))]);
    assert!(
        driver
            .update(&mut host, &tick)
            .unwrap_err()
            .contains("object edit 0")
    );
    assert_eq!(host.accepted_generation(), 0);
}

#[test]
fn driver_owns_rules_identities_and_queues_only_successful_game_changes() {
    let board = RetainedBoard::new(ChessState::initial(), 342).unwrap();
    let original = board.position();
    let ids = board.identities().clone();
    let mut driver = BoardDriver::with_board(board);
    assert!(driver.play_uci("e2e5").unwrap_err().contains("e2e5"));
    assert!(driver.queue().borrow().is_empty());
    driver.play_uci("e2e4").unwrap();
    driver.undo().unwrap();
    assert_eq!(driver.queue().borrow().len(), 2);
    assert_eq!(driver.board().unwrap().position(), original);
    assert_eq!(driver.board().unwrap().identities(), &ids);
    assert!(driver.undo().is_err());
    assert_eq!(driver.queue().borrow().len(), 2);
    let mut editor = BoardDriver::default();
    assert!(editor.play_uci("e2e4").unwrap_err().contains("editor"));
    assert!(editor.undo().unwrap_err().contains("editor"));
}
