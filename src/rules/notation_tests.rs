use super::tests::{KIWIPETE, POSITION_3, POSITION_4, POSITION_5};
use super::*;

#[test]
fn every_legal_move_round_trips_through_san_and_uci() {
    for fen in [
        INITIAL_FEN,
        KIWIPETE,
        POSITION_3,
        POSITION_4,
        POSITION_5,
        "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20",
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        for chess_move in state.legal_moves() {
            let san = state.to_san(chess_move).unwrap();
            assert_eq!(state.parse_san(&san).unwrap(), chess_move, "{fen}: {san}");
            assert_eq!(state.parse_uci(&chess_move.to_uci()).unwrap(), chess_move);
        }
    }
}

#[test]
fn san_disambiguates_files_ranks_and_both() {
    for (fen, uci, san) in [
        ("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1", "b1d2", "Nbd2"),
        ("4k3/8/8/8/8/R7/8/R3K3 w - - 0 1", "a1a2", "R1a2"),
        ("8/7k/8/8/8/Q7/8/Q1Q4K w - - 0 1", "a1c3", "Qa1c3"),
        ("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", "e1g1", "O-O"),
        ("4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1", "e1c1", "O-O-O"),
        ("1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7b8q", "axb8=Q+"),
        ("1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7b8n", "axb8=N"),
        ("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", "g6g7", "Qg7#"),
        ("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20", "e5d6", "exd6"),
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        let chess_move = state.parse_uci(uci).unwrap();
        assert_eq!(state.to_san(chess_move).unwrap(), san);
        assert_eq!(state.parse_san(san).unwrap(), chess_move);
    }
}

#[test]
fn pinned_piece_does_not_require_san_disambiguation() {
    let state = ChessState::from_fen("k3r3/8/8/8/8/8/4N3/4K1N1 w - - 0 1").unwrap();
    assert_eq!(
        state.to_san(state.parse_uci("g1f3").unwrap()).unwrap(),
        "Nf3"
    );
}

#[test]
fn san_rejects_ambiguous_moves_and_incorrect_suffixes() {
    let state = ChessState::from_fen("4k3/8/8/8/8/8/8/1N2KN2 w - - 0 1").unwrap();
    for san in ["Nd2", "Nbd2+", "Nbd2#", "Nbd2!", "Nbd9", "Nbd2 "] {
        assert!(state.parse_san(san).is_err(), "accepted {san}");
    }
}
