use super::tests::{KIWIPETE, POSITION_3, POSITION_4, POSITION_5};
use super::*;

fn positions() -> [&'static str; 10] {
    [
        INITIAL_FEN,
        KIWIPETE,
        POSITION_3,
        POSITION_4,
        POSITION_5,
        "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20",
        "4k3/8/8/8/3Pp3/8/8/4K3 b - d3 0 1",
        "r3k2r/8/8/8/8/8/8/R3K2R b KQkq - 0 1",
        "1r2k3/P7/8/8/8/8/8/4K3 w - - 17 1",
        "4k3/8/8/8/8/8/p7/1R2K3 b - - 0 1",
    ]
}

#[test]
fn direct_masks_match_copy_make_filter_in_order() {
    let mut random = 0x1975_03ab_5d39_f864_u64;
    let mut checked = 0;
    for fen in positions() {
        for _ in 0..8 {
            let mut state = ChessState::from_fen(fen).unwrap();
            let original = state.clone();
            let mut plies = 0;
            for _ in 0..128 {
                let moves = state.legal_moves();
                assert_eq!(
                    moves,
                    state.core.reference_legal_moves(),
                    "{}",
                    state.to_fen()
                );
                checked += 1;
                if moves.is_empty() {
                    break;
                }
                random ^= random << 13;
                random ^= random >> 7;
                random ^= random << 17;
                let choice = usize::try_from(random % u64::try_from(moves.len()).unwrap()).unwrap();
                state.play_generated(moves[choice]).unwrap();
                assert_eq!(
                    state.position_hash(),
                    state.core.zobrist(),
                    "{}",
                    state.to_fen()
                );
                plies += 1;
            }
            for _ in 0..plies {
                state.undo().unwrap();
            }
            assert_eq!(state, original);
        }
    }
    assert!(checked > 8000, "compared {checked} ordered move lists");
}

#[test]
fn changed_square_undo_and_incremental_hash_cover_special_moves() {
    for fen in positions() {
        let mut state = ChessState::from_fen(fen).unwrap();
        let original = state.clone();
        for chess_move in state.legal_moves() {
            let expected = state.core.after(chess_move);
            state.play_generated(chess_move).unwrap();
            assert_eq!(state.core, expected);
            assert_eq!(
                state.position_hash(),
                state.core.zobrist(),
                "{fen} {}",
                chess_move.to_uci()
            );
            assert_eq!(
                state.position_hash(),
                ChessState::from_fen(&state.to_fen())
                    .unwrap()
                    .position_hash()
            );
            state.undo().unwrap();
            assert_eq!(state, original);
        }
    }
}

#[test]
fn precomputed_targets_preserve_mailbox_order() {
    use super::attacks::{KINGS, KNIGHTS, PAWNS};
    use super::board::{KING_STEPS, KNIGHT_STEPS, offset};
    for (table, steps) in [
        (&KNIGHTS, &KNIGHT_STEPS[..]),
        (&KINGS, &KING_STEPS[..]),
        (&PAWNS[0], &[15, 17][..]),
        (&PAWNS[1], &[-17, -15][..]),
    ] {
        for from in (0_u8..128).filter(|from| from & 0x88 == 0) {
            let expected: Vec<_> = steps
                .iter()
                .filter_map(|&step| offset(from, step))
                .collect();
            assert_eq!(
                table[usize::from(from)].iter().collect::<Vec<_>>(),
                expected
            );
        }
    }
}
