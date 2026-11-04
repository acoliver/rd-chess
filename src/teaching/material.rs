use super::board::{material, piece, value, victim};
use crate::Side;
use crate::rules::{ChessState, LegalMove};

/// Name a piece only for a clean, single-piece gain. Exchanges, multiple
/// captures and promotions use the net pawn value, never the gross victim.
pub(super) fn reward(state: &ChessState, line: &[LegalMove], side: Side, gain: i32) -> String {
    let mut cursor = state.clone();
    let mut captured = Vec::new();
    let mut clean = true;
    for &mv in line {
        let mover = cursor.side_to_move();
        if let Some((_, kind)) = victim(&cursor, mv) {
            if mover == side {
                captured.push(kind);
            } else {
                clean = false;
            }
        }
        clean &= mv.promotion().is_none();
        cursor.play(mv).expect("verified material line");
    }
    if clean && captured.len() == 1 && value(captured[0]) == gain {
        return format!("a {}", piece(captured[0]));
    }
    pawns(gain)
}

pub(super) fn line_reward(state: &ChessState, line: &[LegalMove]) -> String {
    let side = state.side_to_move();
    let mut after = state.clone();
    for &mv in line {
        after.play(mv).expect("verified material line");
    }
    reward(
        state,
        line,
        side,
        material(&after, side) - material(state, side),
    )
}

fn pawns(gain: i32) -> String {
    let tenths = (i64::from(gain).abs() + 5) / 10;
    let whole = tenths / 10;
    let fraction = tenths % 10;
    if fraction == 0 {
        let unit = if whole == 1 { "pawn" } else { "pawns" };
        format!("about {whole} {unit}")
    } else {
        format!("about {whole}.{fraction} pawns")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_matching_value_does_not_name_a_piece_after_a_swap_or_promotion() {
        for (fen, moves, gain, expected) in [
            (
                "3r3k/8/8/3p4/4P3/8/8/3R2K1 w - - 0 1",
                "e4d5 d8d5 d1d5",
                500,
                "about 5 pawns",
            ),
            (
                "1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1",
                "a7b8q",
                1350,
                "about 13.5 pawns",
            ),
        ] {
            let state = ChessState::from_fen(fen).unwrap();
            let mut cursor = state.clone();
            let line: Vec<_> = moves
                .split_whitespace()
                .map(|uci| {
                    let mv = cursor.parse_uci(uci).unwrap();
                    cursor.play(mv).unwrap();
                    mv
                })
                .collect();
            assert_eq!(
                material(&cursor, state.side_to_move()) - material(&state, state.side_to_move()),
                gain
            );
            assert_eq!(reward(&state, &line, state.side_to_move(), gain), expected);
        }
    }

    #[test]
    fn pawn_values_round_to_at_most_one_decimal() {
        for (gain, expected) in [
            (100, "about 1 pawn"),
            (200, "about 2 pawns"),
            (220, "about 2.2 pawns"),
            (335, "about 3.4 pawns"),
            (-850, "about 8.5 pawns"),
        ] {
            assert_eq!(pawns(gain), expected);
        }
    }
}
