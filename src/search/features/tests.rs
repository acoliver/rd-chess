use super::*;

#[test]
fn pawn_index_matches_direct_definition_through_seeded_legal_play() {
    for seed in 1_u64..=6 {
        let mut state = ChessState::initial();
        let mut random = seed;
        for _ in 0..80 {
            let features = Features::new(&state);
            for (square, side, kind) in features.men() {
                if kind == PieceKind::Pawn {
                    assert_eq!(
                        features.pawn_structure(square, side),
                        reference(&features, square, side)
                    );
                }
            }
            let moves = state.legal_moves();
            if moves.is_empty() {
                break;
            }
            random = random
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1);
            let offset = usize::try_from(random % u64::try_from(moves.len()).unwrap()).unwrap();
            state.play(moves[offset]).unwrap();
        }
    }
}

fn reference(features: &Features, from: Square, side: Side) -> Term {
    let pawns: Vec<_> = features
        .men()
        .filter(|(_, _, kind)| *kind == PieceKind::Pawn)
        .collect();
    let doubled = pawns.iter().any(|&(other, colour, _)| {
        colour == side && other.file == from.file && other.rank < from.rank
    });
    let isolated = !pawns
        .iter()
        .any(|&(other, colour, _)| colour == side && other.file.abs_diff(from.file) == 1);
    let passed = !pawns.iter().any(|&(other, colour, _)| {
        colour != side
            && other.file.abs_diff(from.file) <= 1
            && if side == Side::Ivory {
                other.rank > from.rank
            } else {
                other.rank < from.rank
            }
    });
    let rank = i32::from(if side == Side::Ivory {
        from.rank
    } else {
        7 - from.rank
    });
    Term {
        middlegame: -12 * i32::from(doubled) - 10 * i32::from(isolated)
            + 3 * rank * rank * i32::from(passed),
        endgame: -18 * i32::from(doubled) - 14 * i32::from(isolated)
            + 7 * rank * rank * i32::from(passed),
    }
}
