#[test]
fn generated_application_matches_public_validation_and_exact_undo() {
    for fen in [INITIAL_FEN, KIWIPETE, POSITION_3, POSITION_4, POSITION_5] {
        let state = ChessState::from_fen(fen).unwrap();
        for chess_move in state.legal_moves() {
            let mut validated = state.clone();
            let mut generated = state.clone();
            validated.play(chess_move).unwrap();
            generated.play_generated(chess_move).unwrap();
            assert_eq!(generated, validated);
            generated.undo().unwrap();
            assert_eq!(generated, state);
        }
    }
}

use super::*;

// Verified against https://www.chessprogramming.org/Perft_Results on 2026-11-04.
// Only counters are appended to the wiki's four-field Kiwipete position.
pub(super) const KIWIPETE: &str =
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
pub(super) const POSITION_3: &str = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
pub(super) const POSITION_4: &str =
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
pub(super) const POSITION_5: &str = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8";

fn perft(core: &Core, depth: u8) -> u64 {
    if depth == 0 {
        return 1;
    }
    let moves = core.legal_moves();
    if depth == 1 {
        return u64::try_from(moves.len()).unwrap();
    }
    moves
        .into_iter()
        .map(|chess_move| perft(&core.after(chess_move), depth - 1))
        .sum()
}

fn check_perft(state: &ChessState, expected: &[u64]) {
    for (index, &nodes) in expected.iter().enumerate() {
        let depth = u8::try_from(index + 1).unwrap();
        assert_eq!(perft(&state.core, depth), nodes, "depth {depth}");
    }
}

#[test]
fn initial_perft_through_depth_four() {
    check_perft(&ChessState::initial(), &[20, 400, 8902, 197_281]);
}

#[test]
fn kiwipete_perft() {
    check_perft(
        &ChessState::from_fen(KIWIPETE).unwrap(),
        &[48, 2039, 97_862],
    );
}

#[test]
fn position_three_perft() {
    check_perft(
        &ChessState::from_fen(POSITION_3).unwrap(),
        &[14, 191, 2812, 43_238],
    );
}

#[test]
fn position_four_perft() {
    check_perft(&ChessState::from_fen(POSITION_4).unwrap(), &[6, 264, 9467]);
}

#[test]
fn position_five_perft() {
    check_perft(
        &ChessState::from_fen(POSITION_5).unwrap(),
        &[44, 1486, 62_379],
    );
}

pub(super) fn play(state: &mut ChessState, uci: &str) {
    let chess_move = state.parse_uci(uci).unwrap();
    state.play(chess_move).unwrap();
}

pub(super) fn has_move(state: &ChessState, uci: &str) -> bool {
    state
        .legal_moves()
        .iter()
        .any(|chess_move| chess_move.to_uci() == uci)
}

#[test]
fn record_restores_initial_fen_and_moves_without_mutating_game() {
    for (fen, moves) in [
        (INITIAL_FEN, "e2e4 e7e5 g1f3"),
        (
            "r3k2r/8/8/3pP3/8/8/8/R3K2R w KQkq d6 0 20",
            "e5d6 e8g8 e1c1 a8d8 d6d7 d8d7",
        ),
        (
            "r3k2r/8/8/3pP3/8/8/8/R3K2R w KQkq - 17 20",
            "e1c1 e8g8 d1d5 a8d8 e5e6 d8d5",
        ),
    ] {
        let mut state = ChessState::from_fen(fen).unwrap();
        assert_eq!(state.record(), (fen.to_owned(), Vec::new()));
        let expected_moves: Vec<_> = moves.split_whitespace().map(str::to_owned).collect();
        for uci in &expected_moves {
            play(&mut state, uci);
        }
        let before = state.clone();
        let (initial_fen, recorded_moves) = state.record();
        assert_eq!(initial_fen, fen);
        assert_eq!(recorded_moves, expected_moves);
        assert_eq!(state, before);
        let mut replayed = ChessState::from_fen(&initial_fen).unwrap();
        for uci in &recorded_moves {
            play(&mut replayed, uci);
        }
        assert_eq!(replayed, state);
        state.undo().unwrap();
        let (initial_fen, recorded_moves) = state.record();
        assert_eq!(initial_fen, fen);
        assert_eq!(recorded_moves, expected_moves[..expected_moves.len() - 1]);
        for _ in 0..recorded_moves.len() {
            state.undo().unwrap();
        }
        assert_eq!(state.record(), (fen.to_owned(), Vec::new()));
    }
}

#[test]
fn fen_round_trip_preserves_fields_and_checks() {
    for fen in [INITIAL_FEN, KIWIPETE, POSITION_3, POSITION_4, POSITION_5] {
        let state = ChessState::from_fen(fen).unwrap();
        assert_eq!(state.to_fen(), fen);
        assert_eq!(state.repetition_history(), &[state.position_hash()]);
    }
    let state = ChessState::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20").unwrap();
    assert_eq!(state.to_fen(), "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20");
}

#[test]
fn malformed_or_illegal_fens_are_rejected() {
    for fen in [
        "8/8/8/8/8/8/8/8 w - - 0 1",
        "4k3/8/8/8/8/8/8/3KK3 w - - 0 1",
        "8/8/8/8/8/8/4k3/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/P3K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/4K2P w - - 0 1",
        "4k3/8/8/8/8/8/4R3/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w K - 0 1",
        "4k2r/8/8/8/8/8/8/4K3 b KK - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w - d6 0 1",
        "4k3/8/8/3p4/8/8/8/4K3 w - d3 0 1",
        "4k3/3n4/8/3p4/8/8/8/4K3 w - d6 0 1",
        "4k3/8/8/3p4/8/8/8/4K3 w - d6 1 1",
        "4k3/8/8/8/8/8/8/4K3 x - - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w - - -1 1",
        "4k3/8/8/8/8/8/8/4K3 w - - 0 0",
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1 extra",
        "4k3/8/8/8/8/8/8/4K3 w - -",
        "4k3/8/8/8/8/8/8/44K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/3zK3 w - - 0 1",
    ] {
        assert!(ChessState::from_fen(fen).is_err(), "accepted {fen}");
    }
}

#[test]
fn castling_blocked_in_check_or_through_check() {
    for (fen, king_side, queen_side) in [
        ("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1", true, true),
        ("4k3/8/8/8/8/8/8/R3KB1R w KQ - 0 1", false, true),
        ("4k3/8/8/8/8/8/8/RN2K2R w KQ - 0 1", true, false),
        ("4kr2/8/8/8/8/8/8/R3K2R w KQ - 0 1", false, true),
        ("4k1r1/8/8/8/8/8/8/R3K2R w KQ - 0 1", false, true),
        ("1k2r3/8/8/8/8/8/8/R3K2R w KQ - 0 1", false, false),
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        assert_eq!(has_move(&state, "e1g1"), king_side, "{fen}");
        assert_eq!(has_move(&state, "e1c1"), queen_side, "{fen}");
    }
}

#[test]
fn rook_return_and_capture_do_not_restore_rights() {
    let mut state = ChessState::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    for uci in ["h1h2", "h8h7", "h2h1", "h7h8"] {
        play(&mut state, uci);
    }
    assert!(!has_move(&state, "e1g1"));
    assert!(has_move(&state, "e1c1"));
    play(&mut state, "a1a8");
    assert_eq!(state.castling_rights().to_string(), "-");
}

#[test]
fn pinned_en_passant_is_not_legal() {
    let pinned = ChessState::from_fen("4k3/8/8/r4pPK/8/8/8/8 w - f6 0 2").unwrap();
    assert!(!has_move(&pinned, "g5f6"));
    let mut free = ChessState::from_fen("4k3/8/8/5pPK/8/8/8/8 w - f6 0 2").unwrap();
    play(&mut free, "g5f6");
    assert!(free.piece_at(Square::new(5, 4).unwrap()).is_none());
    assert_eq!(free.halfmove_clock(), 0);
}

#[test]
fn promotions_include_all_four_capture_underpromotions() {
    let mut state = ChessState::from_fen("1r2k3/P7/8/8/8/8/8/4K3 w - - 17 1").unwrap();
    for suffix in ["q", "r", "b", "n"] {
        assert!(has_move(&state, &format!("a7b8{suffix}")));
        assert!(has_move(&state, &format!("a7a8{suffix}")));
    }
    assert!(!has_move(&state, "a7b8"));
    play(&mut state, "a7b8n");
    assert_eq!(
        state.piece_at(Square::new(1, 7).unwrap()),
        Some((Side::Ivory, PieceKind::Knight))
    );
    assert_eq!(state.halfmove_clock(), 0);
}

#[test]
fn double_check_only_allows_king_moves() {
    let state = ChessState::from_fen("4r1k1/8/8/8/1b6/8/8/R3K3 w - - 0 1").unwrap();
    assert!(state.in_check());
    assert_ne!(state.legal_moves().len(), 0);
    assert!(
        state
            .legal_moves()
            .iter()
            .all(|chess_move| chess_move.from() == Square::new(4, 0).unwrap())
    );
}

#[test]
fn undo_restores_all_state_for_every_special_move() {
    for fen in [
        INITIAL_FEN,
        KIWIPETE,
        POSITION_4,
        POSITION_5,
        "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20",
    ] {
        let mut state = ChessState::from_fen(fen).unwrap();
        let original = state.clone();
        for chess_move in state.legal_moves() {
            state.play(chess_move).unwrap();
            state.undo().unwrap();
            assert_eq!(state, original, "{}", chess_move.to_uci());
        }
        assert_eq!(state.undo(), Err(MoveError::NoHistory));
    }
}

#[test]
fn stale_moves_are_revalidated_without_mutation() {
    let mut state = ChessState::initial();
    let obsolete = state.parse_uci("e2e4").unwrap();
    play(&mut state, "d2d4");
    let before = state.clone();
    assert_eq!(state.play(obsolete), Err(MoveError::IllegalMove));
    assert_eq!(state, before);
    for text in ["a0a2", "e2e9", "e2e4q", "e2e4x", "é2e4", "e2e4\n"] {
        assert!(state.parse_uci(text).is_err());
    }
}

#[test]
fn seeded_legal_sequences_restore_history_and_validate_fen() {
    let mut random = 0x7c36_a192_d03b_485f_u64;
    for _ in 0..8 {
        let mut state = ChessState::initial();
        let original = state.clone();
        let mut plies = 0;
        for _ in 0..96 {
            let moves = state.legal_moves();
            if moves.is_empty() {
                break;
            }
            random ^= random << 13;
            random ^= random >> 7;
            random ^= random << 17;
            let index = usize::try_from(random % u64::try_from(moves.len()).unwrap()).unwrap();
            state.play(moves[index]).unwrap();
            let loaded = ChessState::from_fen(&state.to_fen()).unwrap();
            assert_eq!(loaded.position_hash(), state.position_hash());
            assert_eq!(loaded.legal_moves(), state.legal_moves());
            assert_eq!(state.repetition_history().len(), plies + 2);
            plies += 1;
        }
        for _ in 0..plies {
            state.undo().unwrap();
        }
        assert_eq!(state, original);
    }
}

#[test]
fn scene_document_conversion_keeps_placement_and_stable_ids() {
    let mut state = ChessState::initial();
    let before = state.to_position(42);
    let pawn_id = before
        .pieces
        .iter()
        .find(|piece| piece.square == Square::new(4, 1).unwrap())
        .unwrap()
        .id
        .clone();
    play(&mut state, "e2e4");
    let document = state.to_position(42);
    document.validate().unwrap();
    assert_eq!(document.seed, 42);
    assert_eq!(document.pieces.len(), 32);
    assert!(
        document
            .pieces
            .iter()
            .any(|piece| piece.id == pawn_id && piece.square == Square::new(4, 3).unwrap())
    );
    assert_eq!(
        Position::from_json(&document.to_json().unwrap()).unwrap(),
        document
    );
}
