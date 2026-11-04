use super::tests::{has_move, play};
use super::*;

#[test]
fn rules_generated_mate_builds_the_existing_cpu_scene_plan() {
    let mut state = ChessState::initial();
    for uci in ["f2f3", "e7e5", "g2g4", "d8h4"] {
        play(&mut state, uci);
    }
    assert_eq!(state.status(), GameStatus::Checkmate { winner: Side::Onyx });
    let document = state.to_position(12);
    let scene = crate::render_scene(&document).unwrap();
    let objects = scene.geometry().objects();
    let mut index = 4;
    for piece in &document.pieces {
        assert_eq!(
            objects[index].transform().position_millimeters,
            [
                i64::from(piece.square.file) * 1000 - 3500,
                i64::from(piece.square.rank) * 1000 - 3500,
                50,
            ]
        );
        index += if piece.kind == PieceKind::Knight {
            2
        } else {
            1
        };
    }
    assert_eq!(objects.len(), index);
}

#[test]
fn fen_errors_identify_the_failed_contract() {
    for (fen, expected) in [
        ("8/8/8/8/8/8/8/8 w - - 0 1", FenError::Kings),
        ("4k3/8/8/8/8/8/4K3/4k3 w - - 0 1", FenError::Kings),
        ("8/8/8/8/8/8/4k3/4K3 w - - 0 1", FenError::AdjacentKings),
        ("4k3/8/8/8/8/8/8/P3K3 w - - 0 1", FenError::PawnRank),
        (
            "4k3/8/8/8/8/8/4R3/4K3 w - - 0 1",
            FenError::PreviousSideInCheck,
        ),
        ("4k3/8/8/8/8/8/8/4K3 w K - 0 1", FenError::Castling),
        ("4k3/8/8/3P4/8/8/8/4K3 w - d6 0 2", FenError::EnPassant),
        ("4k3/8/3B4/3p4/8/8/8/4K3 w - d6 0 2", FenError::EnPassant),
        ("4k3/8/8/3p4/8/8/8/4K3 w - d6 0 1", FenError::EnPassant),
        ("k3r3/8/8/3p4/8/8/8/4K3 w - d6 0 2", FenError::EnPassant),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 0", FenError::Counters),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0 4294967296", FenError::Counters),
        ("4k3/8/8/8/8/8/8/4K3 w - - 0", FenError::Fields),
        ("4k3/8/8/8/8/8/PPPPPPPP/QQ2K3 w - - 0 1", FenError::Material),
        ("4k3/8/8/8/8/P7/PPPPPPPP/4K3 w - - 0 1", FenError::Material),
        (
            "4k3/8/8/8/8/8/PPPPPPPP/B1B1K3 w - - 0 1",
            FenError::Material,
        ),
    ] {
        assert_eq!(ChessState::from_fen(fen), Err(expected), "{fen}");
    }
}

#[test]
fn en_passant_can_evade_check_but_not_expose_its_own_king() {
    let mut evasion = ChessState::from_fen("k7/8/8/3pP3/4K3/8/8/8 w - d6 0 2").unwrap();
    assert!(evasion.in_check());
    play(&mut evasion, "e5d6");
    assert!(!evasion.core.in_check(Side::Ivory));
    let pinned = ChessState::from_fen("k3r3/8/8/3pP3/8/8/8/4K3 w - d6 0 2").unwrap();
    assert!(!has_move(&pinned, "e5d6"));
}

#[test]
fn black_special_moves_and_ep_expiration() {
    let mut ep = ChessState::from_fen("4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 1").unwrap();
    let original = ep.clone();
    play(&mut ep, "e4d3");
    assert_eq!(
        ep.piece_at(Square::new(3, 2).unwrap()),
        Some((Side::Onyx, PieceKind::Pawn))
    );
    assert_eq!(ep.piece_at(Square::new(3, 3).unwrap()), None);
    ep.undo().unwrap();
    assert_eq!(ep, original);
    play(&mut ep, "e8f8");
    assert_eq!(ep.en_passant_square(), None);
    let mut castle = ChessState::from_fen("r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1").unwrap();
    play(&mut castle, "e8c8");
    assert_eq!(
        castle.piece_at(Square::new(3, 7).unwrap()),
        Some((Side::Onyx, PieceKind::Rook))
    );
    assert_eq!(castle.castling_rights().to_string(), "KQ");
    let mut promotion = ChessState::from_fen("4k3/8/8/8/8/8/p7/1R2K3 b - - 0 1").unwrap();
    play(&mut promotion, "a2b1b");
    assert_eq!(
        promotion.piece_at(Square::new(1, 0).unwrap()),
        Some((Side::Onyx, PieceKind::Bishop))
    );
}

#[test]
fn king_return_does_not_restore_either_right() {
    let mut state = ChessState::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    for uci in ["e1f1", "e8f8", "f1e1", "f8e8"] {
        play(&mut state, uci);
    }
    assert_eq!(state.castling_rights().to_string(), "-");
    assert!(!has_move(&state, "e1g1"));
    assert!(!has_move(&state, "e1c1"));
}

#[test]
fn overflowing_counters_fail_without_changing_any_state() {
    for fen in [
        "4k3/8/8/8/8/8/8/R3K3 w - - 4294967295 1",
        "4k3/8/8/8/8/8/8/R3K3 b - - 0 4294967295",
    ] {
        let mut state = ChessState::from_fen(fen).unwrap();
        let original = state.clone();
        let chess_move = state.legal_moves()[0];
        assert_eq!(state.play(chess_move), Err(MoveError::CounterOverflow));
        assert_eq!(state, original);
    }
}

#[test]
fn mate_on_the_one_hundred_fiftieth_ply_has_precedence() {
    let mut state = ChessState::from_fen("7k/8/5KQ1/8/8/8/8/8 w - - 149 80").unwrap();
    play(&mut state, "g6g7");
    assert_eq!(state.halfmove_clock(), 150);
    assert_eq!(
        state.status(),
        GameStatus::Checkmate {
            winner: Side::Ivory
        }
    );
}

#[test]
fn double_push_records_fen_ep_even_without_a_capturer() {
    let mut state = ChessState::initial();
    play(&mut state, "e2e4");
    assert_eq!(
        state.to_fen(),
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1"
    );
    assert_eq!(state.en_passant_square(), Some(Square::new(4, 2).unwrap()));
    assert_eq!(state.side_to_move(), Side::Onyx);
    play(&mut state, "g8f6");
    assert_eq!(state.en_passant_square(), None);
}

#[test]
fn invalid_editor_square_never_indexes_the_mailbox() {
    let state = ChessState::initial();
    for location in [
        Square { file: 8, rank: 0 },
        Square {
            file: 255,
            rank: 255,
        },
    ] {
        assert_eq!(state.piece_at(location), None);
    }
}
