use super::evaluation::Term;
use crate::rules::ChessState;
use crate::{PieceKind, Side, Square};

const DIAGONALS: [(i16, i16); 4] = [(1, 1), (1, -1), (-1, 1), (-1, -1)];
const STRAIGHTS: [(i16, i16); 4] = [(1, 0), (-1, 0), (0, 1), (0, -1)];
const KNIGHTS: [(i16, i16); 8] = [
    (1, 2),
    (2, 1),
    (-1, 2),
    (-2, 1),
    (1, -2),
    (2, -1),
    (-1, -2),
    (-2, -1),
];

pub(super) struct Features {
    board: [Option<(Side, PieceKind)>; 64],
    attacks: [[u8; 64]; 2],
    mobility: [i32; 64],
}

pub(super) fn index(square: Square) -> usize {
    usize::from(square.rank) * 8 + usize::from(square.file)
}

fn square(index: u8) -> Square {
    Square {
        file: index % 8,
        rank: index / 8,
    }
}

fn shifted(square: Square, delta: (i16, i16)) -> Option<Square> {
    let file = u8::try_from(i16::from(square.file) + delta.0).ok()?;
    let rank = u8::try_from(i16::from(square.rank) + delta.1).ok()?;
    (file < 8 && rank < 8).then_some(Square { file, rank })
}

impl Features {
    pub fn new(state: &ChessState) -> Self {
        let board = std::array::from_fn(|offset| {
            state.piece_at(square(u8::try_from(offset).expect("board offset")))
        });
        let mut result = Self {
            board,
            attacks: [[0; 64]; 2],
            mobility: [0; 64],
        };
        for offset in 0..64 {
            if let Some((side, kind)) = result.board[usize::from(offset)] {
                result.piece_attacks(square(offset), side, kind);
            }
        }
        result
    }

    pub fn men(&self) -> impl Iterator<Item = (Square, Side, PieceKind)> + '_ {
        (0..64).filter_map(|offset| {
            self.board[usize::from(offset)].map(|(side, kind)| (square(offset), side, kind))
        })
    }

    fn piece_attacks(&mut self, from: Square, side: Side, kind: PieceKind) {
        match kind {
            PieceKind::Pawn => {
                let advance = if side == Side::Ivory { 1 } else { -1 };
                self.rays(from, side, &[(-1, advance), (1, advance)], false);
            }
            PieceKind::Knight => self.rays(from, side, &KNIGHTS, false),
            PieceKind::Bishop => self.rays(from, side, &DIAGONALS, true),
            PieceKind::Rook => self.rays(from, side, &STRAIGHTS, true),
            PieceKind::Queen => {
                self.rays(from, side, &DIAGONALS, true);
                self.rays(from, side, &STRAIGHTS, true);
            }
            PieceKind::King => {
                self.rays(from, side, &DIAGONALS, false);
                self.rays(from, side, &STRAIGHTS, false);
            }
        }
    }

    fn rays(&mut self, from: Square, side: Side, directions: &[(i16, i16)], sliding: bool) {
        for &delta in directions {
            let mut cursor = shifted(from, delta);
            while let Some(to) = cursor {
                let offset = index(to);
                self.attacks[usize::from(side == Side::Onyx)][offset] += 1;
                if self.board[offset].is_none_or(|(occupant, _)| occupant != side) {
                    self.mobility[index(from)] += 1;
                }
                if !sliding || self.board[offset].is_some() {
                    break;
                }
                cursor = shifted(to, delta);
            }
        }
    }

    /// Attack-space mobility, not legal-move count. Pins and king safety remain
    /// the rules core's responsibility. Pawn and king mobility are not rewarded.
    pub fn mobility(&self, from: Square, kind: PieceKind) -> Term {
        let weight = match kind {
            PieceKind::Knight | PieceKind::Bishop => 4,
            PieceKind::Rook => 2,
            PieceKind::Queen => 1,
            _ => 0,
        };
        let value = self.mobility[index(from)] * weight;
        Term {
            middlegame: value,
            endgame: value,
        }
    }

    /// Doubled -12/-18 per extra pawn, isolated -10/-14 per pawn, passed pawn
    /// +3*r*r/+7*r*r with r measured from that pawn's home side.
    pub fn pawn_structure(&self, from: Square, side: Side) -> Term {
        let pawns: Vec<_> = self
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

    /// Shield pawns on adjacent files one/two ranks ahead add 12/6 MG points.
    /// Enemy attacks on the king ring cost 8 MG and 2 EG per attack.
    pub fn king_safety(&self, king: Square, side: Side) -> Term {
        let step = if side == Side::Ivory { 1 } else { -1 };
        let mut shield = 0;
        let mut attackers = 0;
        for file in -1..=1 {
            for rank in -1..=1 {
                if let Some(target) = shifted(king, (file, rank)) {
                    attackers +=
                        i32::from(self.attacks[usize::from(side == Side::Ivory)][index(target)]);
                }
            }
            for (distance, bonus) in [(1, 12), (2, 6)] {
                if let Some(target) = shifted(king, (file, step * distance))
                    && self.board[index(target)] == Some((side, PieceKind::Pawn))
                {
                    shield += bonus;
                }
            }
        }
        Term {
            middlegame: shield - attackers * 8,
            endgame: -attackers * 2,
        }
    }
}
