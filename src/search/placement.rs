use super::Term;
use crate::{PieceKind, Side, Square};

// Authored formulas and units are documented in evaluation.rs. Generate the
// six white-oriented 64-square tables at compile time, not in the node loop.
const TABLES: [[Term; 64]; 6] = tables();

const fn tables() -> [[Term; 64]; 6] {
    let mut tables = [[Term {
        middlegame: 0,
        endgame: 0,
    }; 64]; 6];
    let mut kind = 0;
    while kind < 6 {
        let mut offset = 0;
        let mut file = 0;
        let mut rank = 0;
        while offset < 64 {
            tables[kind][offset] = entry(kind, file, rank);
            file += 1;
            if file == 8 {
                file = 0;
                rank += 1;
            }
            offset += 1;
        }
        kind += 1;
    }
    tables
}

const fn entry(kind: usize, file: i32, rank: i32) -> Term {
    let centre =
        (if file < 4 { file } else { 7 - file }) + (if rank < 4 { rank } else { 7 - rank });
    let (middlegame, endgame) = match kind {
        0 => (rank * 6 + centre * 2, rank * 12),
        1 | 2 => (centre * 5, centre * 4),
        3 => (rank * 2, rank * 3),
        4 => (centre * 2, centre * 3),
        _ => (-centre * 8 - rank * 3, centre * 10),
    };
    Term {
        middlegame,
        endgame,
    }
}

pub(super) fn at(kind: PieceKind, square: Square, side: Side) -> Term {
    let kind = match kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    };
    let rank = if side == Side::Ivory {
        square.rank
    } else {
        7 - square.rank
    };
    TABLES[kind][usize::from(rank) * 8 + usize::from(square.file)]
}
