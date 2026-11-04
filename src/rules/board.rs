use super::{CastlingRights, LegalMove, PieceKind, Side, Square};

pub(super) const KNIGHT_STEPS: [i16; 8] = [33, 31, 18, 14, -14, -18, -31, -33];
pub(super) const DIAGONALS: [i16; 4] = [17, 15, -15, -17];
pub(super) const ORTHOGONALS: [i16; 4] = [16, 1, -1, -16];
pub(super) const KING_STEPS: [i16; 8] = [17, 16, 15, 1, -1, -15, -16, -17];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Man {
    pub side: Side,
    pub kind: PieceKind,
    pub id: u8,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Core {
    pub board: [Option<Man>; 128],
    pub kings: [u8; 2],
    pub side: Side,
    pub rights: CastlingRights,
    pub ep: Option<u8>,
}

pub(super) fn opposite(side: Side) -> Side {
    match side {
        Side::Ivory => Side::Onyx,
        Side::Onyx => Side::Ivory,
    }
}

pub(super) fn side_index(side: Side) -> usize {
    usize::from(side == Side::Onyx)
}

pub(super) fn pawn_step(side: Side) -> i16 {
    if side == Side::Ivory { 16 } else { -16 }
}

pub(super) fn home_rank(side: Side) -> u8 {
    if side == Side::Ivory { 0 } else { 7 }
}

pub(super) fn square(location: u8) -> Square {
    Square {
        file: location & 7,
        rank: location >> 4,
    }
}

pub(super) fn index(location: Square) -> Option<u8> {
    (location.file < 8 && location.rank < 8).then(|| location.rank * 16 + location.file)
}

pub(super) fn offset(location: u8, step: i16) -> Option<u8> {
    u8::try_from(i16::from(location) + step)
        .ok()
        .filter(|value| value & 0x88 == 0)
}

impl Core {
    pub fn at(&self, location: u8) -> Option<Man> {
        self.board[usize::from(location)]
    }

    pub fn men(&self) -> impl Iterator<Item = (u8, Man)> + '_ {
        (0_u8..128)
            .filter(|location| location & 0x88 == 0)
            .filter_map(|location| self.at(location).map(|man| (location, man)))
    }

    pub fn in_check(&self, side: Side) -> bool {
        self.attacked(self.kings[side_index(side)], opposite(side))
    }

    pub fn attacked(&self, target: u8, by: Side) -> bool {
        [-1, 1]
            .iter()
            .any(|file| self.has_attacker(target, -pawn_step(by) + file, by, PieceKind::Pawn))
            || KNIGHT_STEPS
                .iter()
                .any(|&step| self.has_attacker(target, step, by, PieceKind::Knight))
            || KING_STEPS
                .iter()
                .any(|&step| self.has_attacker(target, step, by, PieceKind::King))
            || self.ray_attacked(target, by, &DIAGONALS, PieceKind::Bishop)
            || self.ray_attacked(target, by, &ORTHOGONALS, PieceKind::Rook)
    }

    fn has_attacker(&self, target: u8, step: i16, by: Side, kind: PieceKind) -> bool {
        offset(target, step)
            .and_then(|location| self.at(location))
            .is_some_and(|man| man.side == by && man.kind == kind)
    }

    fn ray_attacked(&self, target: u8, by: Side, steps: &[i16], kind: PieceKind) -> bool {
        steps.iter().any(|&step| {
            let mut cursor = offset(target, step);
            while let Some(location) = cursor {
                if let Some(man) = self.at(location) {
                    return man.side == by && (man.kind == kind || man.kind == PieceKind::Queen);
                }
                cursor = offset(location, step);
            }
            false
        })
    }

    pub fn is_capture(&self, chess_move: LegalMove) -> bool {
        self.at(chess_move.to).is_some() || self.is_en_passant(chess_move)
    }

    pub fn is_en_passant(&self, chess_move: LegalMove) -> bool {
        self.ep == Some(chess_move.to)
            && self
                .at(chess_move.from)
                .is_some_and(|man| man.kind == PieceKind::Pawn)
    }

    /// Internal copy-make for generated moves only. Clocks/history live outside Core.
    pub fn after(mut self, chess_move: LegalMove) -> Self {
        let mut man = self.at(chess_move.from).expect("generated mover exists");
        if self.is_en_passant(chess_move) {
            let victim = offset(chess_move.to, -pawn_step(man.side)).expect("ep victim on board");
            self.board[usize::from(victim)] = None;
        }
        self.ep = None;
        self.rights.remove_corner(chess_move.from);
        self.rights.remove_corner(chess_move.to);
        if man.kind == PieceKind::King {
            self.kings[side_index(man.side)] = chess_move.to;
            self.rights.remove_side(man.side);
            self.move_castling_rook(chess_move);
        }
        if man.kind == PieceKind::Pawn && chess_move.from.abs_diff(chess_move.to) == 32 {
            self.ep = offset(chess_move.from, pawn_step(man.side));
        }
        if let Some(promotion) = chess_move.promotion {
            man.kind = promotion.kind();
        }
        self.board[usize::from(chess_move.from)] = None;
        self.board[usize::from(chess_move.to)] = Some(man);
        self.side = opposite(self.side);
        self
    }

    fn move_castling_rook(&mut self, chess_move: LegalMove) {
        if chess_move.from.abs_diff(chess_move.to) != 2 {
            return;
        }
        let rank_start = chess_move.from & 0x70;
        let (rook_from, rook_to) = if chess_move.to > chess_move.from {
            (rank_start + 7, rank_start + 5)
        } else {
            (rank_start, rank_start + 3)
        };
        self.board[usize::from(rook_to)] = self.board[usize::from(rook_from)].take();
    }
}
