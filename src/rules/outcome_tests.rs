#[test]
fn status_from_current_legal_list_matches_public_outcomes() {
    for fen in [
        INITIAL_FEN,
        "7k/6Q1/6K1/8/8/8/8/8 b - - 150 80",
        "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/R3K3 w - - 100 80",
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        assert_eq!(
            state.status_with_legal(&state.legal_moves()),
            state.status()
        );
    }
}

use super::tests::{has_move, play};
use super::*;

#[test]
fn mate_and_stalemate_take_precedence_over_move_clocks() {
    let mate = ChessState::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 150 80").unwrap();
    assert_eq!(
        mate.status(),
        GameStatus::Checkmate {
            winner: Side::Ivory
        }
    );
    let stale = ChessState::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 150 80").unwrap();
    assert_eq!(stale.status(), GameStatus::Draw(DrawReason::Stalemate));
}

#[test]
fn fifty_move_is_claimable_and_seventy_five_is_automatic() {
    let mut state = ChessState::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 99 50").unwrap();
    assert!(!state.draw_claims().fifty_move);
    let intended = state.parse_uci("a1a2").unwrap();
    assert!(state.claims_after(intended).unwrap().fifty_move);
    play(&mut state, "a1a2");
    assert!(matches!(
        state.status(),
        GameStatus::Ongoing {
            claims: DrawClaims {
                fifty_move: true,
                ..
            },
            ..
        }
    ));
    let automatic = ChessState::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 150 76").unwrap();
    assert_eq!(
        automatic.status(),
        GameStatus::Draw(DrawReason::SeventyFiveMove)
    );
}

#[test]
fn threefold_is_claimable_and_fivefold_is_automatic() {
    let mut state = ChessState::initial();
    for cycle in 1..=4 {
        for uci in ["g1f3", "g8f6", "f3g1"] {
            play(&mut state, uci);
        }
        let intended = state.parse_uci("f6g8").unwrap();
        assert_eq!(state.claims_after(intended).unwrap().threefold, cycle >= 2);
        play(&mut state, "f6g8");
        assert_eq!(state.repetition_count(), cycle + 1);
        assert_eq!(state.draw_claims().threefold, cycle >= 2);
    }
    assert_eq!(
        state.status(),
        GameStatus::Draw(DrawReason::FivefoldRepetition)
    );
    state.undo().unwrap();
    assert!(matches!(state.status(), GameStatus::Ongoing { .. }));
}

#[test]
fn repetition_identity_uses_only_legally_available_en_passant() {
    for (with_ep, without_ep, equal) in [
        (
            "4k3/8/8/3p4/8/8/8/4K3 w - d6 0 2",
            "4k3/8/8/3p4/8/8/8/4K3 w - - 0 2",
            true,
        ),
        (
            "4k3/8/8/r4pPK/8/8/8/8 w - f6 0 2",
            "4k3/8/8/r4pPK/8/8/8/8 w - - 0 2",
            true,
        ),
        (
            "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2",
            "4k3/8/8/3pP3/8/8/8/4K3 w - - 0 2",
            false,
        ),
    ] {
        assert_eq!(
            ChessState::from_fen(with_ep).unwrap().position_hash()
                == ChessState::from_fen(without_ep).unwrap().position_hash(),
            equal
        );
    }
    let rights = ChessState::from_fen("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1").unwrap();
    let none = ChessState::from_fen("4k3/8/8/8/8/8/8/R3K2R w - - 1 4").unwrap();
    assert_ne!(rights.position_hash(), none.position_hash());
    let black = ChessState::from_fen("4k3/8/8/8/8/8/8/R3K2R b - - 1 4").unwrap();
    assert_ne!(none.position_hash(), black.position_hash());
}

#[test]
fn supported_dead_material_cases_and_non_dead_cases() {
    for fen in [
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
        "4k3/8/8/8/8/8/8/3BK3 w - - 0 1",
        "4k3/8/8/8/8/8/8/3NK3 w - - 0 1",
        "2b1k3/8/8/8/8/8/8/3BK3 w - - 0 1",
    ] {
        assert_eq!(
            ChessState::from_fen(fen).unwrap().status(),
            GameStatus::Draw(DrawReason::InsufficientMaterial)
        );
    }
    for fen in [
        "3bk3/8/8/8/8/8/8/3BK3 w - - 0 1",
        "4k3/8/8/8/8/8/8/2NNK3 w - - 0 1",
        "4k3/8/8/8/8/8/P7/4K3 w - - 0 1",
    ] {
        assert!(matches!(
            ChessState::from_fen(fen).unwrap().status(),
            GameStatus::Ongoing { .. }
        ));
    }
}

#[test]
fn pawn_move_capture_and_black_move_update_counters() {
    let mut state = ChessState::from_fen("4k3/8/8/8/1p6/8/P7/4K3 w - - 99 40").unwrap();
    play(&mut state, "a2a3");
    assert_eq!(state.halfmove_clock(), 0);
    assert_eq!(state.fullmove_number(), 40);
    play(&mut state, "b4a3");
    assert_eq!(state.halfmove_clock(), 0);
    assert_eq!(state.fullmove_number(), 41);
}

#[test]
fn castling_moves_both_pieces_and_preserves_identity_on_undo() {
    let mut state = ChessState::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap();
    let original = state.clone();
    play(&mut state, "e1g1");
    assert_eq!(
        state.piece_at(Square::new(5, 0).unwrap()),
        Some((Side::Ivory, PieceKind::Rook))
    );
    assert!(state.piece_at(Square::new(7, 0).unwrap()).is_none());
    assert!(!has_move(&state, "e8g8")); // White's rook now attacks f8.
    state.undo().unwrap();
    assert_eq!(state, original);
}
