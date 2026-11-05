use crate::rules::{ChessState, LegalMove};
use crate::{PieceKind, Side, Square};

pub(super) fn squares() -> impl Iterator<Item = Square> {
    (0..8).flat_map(|rank| (0..8).map(move |file| Square { file, rank }))
}

pub(super) fn name(square: Square) -> String {
    format!("{}{}", char::from(b'a' + square.file), square.rank + 1)
}

pub(super) fn colour(side: Side) -> &'static str {
    if side == Side::Ivory {
        "White"
    } else {
        "Black"
    }
}

pub(super) fn piece(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pawn",
        PieceKind::Knight => "knight",
        PieceKind::Bishop => "bishop",
        PieceKind::Rook => "rook",
        PieceKind::Queen => "queen",
        PieceKind::King => "king",
    }
}

pub(super) fn value(kind: PieceKind) -> i32 {
    match kind {
        PieceKind::Pawn => 100,
        PieceKind::Knight => 320,
        PieceKind::Bishop => 335,
        PieceKind::Rook => 500,
        PieceKind::Queen => 950,
        PieceKind::King => 0,
    }
}

pub(super) fn material(state: &ChessState, side: Side) -> i32 {
    squares()
        .filter_map(|sq| state.piece_at(sq))
        .map(|(owner, kind)| value(kind) * if owner == side { 1 } else { -1 })
        .sum()
}

pub(super) fn victim(state: &ChessState, mv: LegalMove) -> Option<(Square, PieceKind)> {
    state
        .piece_at(mv.to())
        .map(|(_, kind)| (mv.to(), kind))
        .or_else(|| {
            (state
                .piece_at(mv.from())
                .is_some_and(|(_, kind)| kind == PieceKind::Pawn)
                && mv.from().file != mv.to().file)
                .then_some((
                    Square {
                        file: mv.to().file,
                        rank: mv.from().rank,
                    },
                    PieceKind::Pawn,
                ))
        })
}

pub(super) fn shifted(square: Square, step: (i16, i16)) -> Option<Square> {
    let file = u8::try_from(i16::from(square.file) + step.0).ok()?;
    let rank = u8::try_from(i16::from(square.rank) + step.1).ok()?;
    (file < 8 && rank < 8).then_some(Square { file, rank })
}

pub(super) fn direction(from: Square, to: Square) -> (i16, i16) {
    (
        i16::from(to.file) - i16::from(from.file),
        i16::from(to.rank) - i16::from(from.rank),
    )
}

pub(super) fn slider(kind: PieceKind, delta: (i16, i16)) -> bool {
    let diagonal = delta.0.abs() == delta.1.abs();
    let straight = delta.0 == 0 || delta.1 == 0;
    match kind {
        PieceKind::Bishop => diagonal,
        PieceKind::Rook => straight,
        PieceKind::Queen => diagonal || straight,
        _ => false,
    }
}

/// Geometric proposals only. Pins and illegal captures are rejected by the
/// rules-driven verification and exchange trees, never inferred from this map.
pub(super) fn attacks(state: &ChessState, from: Square, to: Square) -> bool {
    let Some((side, kind)) = state.piece_at(from) else {
        return false;
    };
    let delta = direction(from, to);
    if from == to {
        return false;
    }
    match kind {
        PieceKind::Pawn => {
            delta.0.abs() == 1 && delta.1 == if side == Side::Ivory { 1 } else { -1 }
        }
        PieceKind::Knight => matches!((delta.0.abs(), delta.1.abs()), (1, 2) | (2, 1)),
        PieceKind::King => delta.0.abs().max(delta.1.abs()) == 1,
        _ => slider(kind, delta) && clear(state, from, to),
    }
}

fn clear(state: &ChessState, from: Square, to: Square) -> bool {
    let delta = direction(from, to);
    let step = (delta.0.signum(), delta.1.signum());
    let mut cursor = shifted(from, step);
    while let Some(square) = cursor {
        if square == to {
            return true;
        }
        if state.piece_at(square).is_some() {
            return false;
        }
        cursor = shifted(square, step);
    }
    false
}

pub(super) fn captured_in(
    state: &ChessState,
    line: &[LegalMove],
    from: Square,
    target: Square,
) -> bool {
    let mut cursor = state.clone();
    let mut attacker_at = from;
    let mut target_at = target;
    for &mv in line {
        let captured = victim(&cursor, mv).map(|(sq, _)| sq);
        if captured == Some(target_at) {
            return mv.from() == attacker_at;
        }
        if captured == Some(attacker_at) {
            return false;
        }
        if mv.from() == attacker_at {
            attacker_at = mv.to();
        }
        if mv.from() == target_at {
            target_at = mv.to();
        }
        cursor.play(mv).expect("verified line is legal");
    }
    false
}
