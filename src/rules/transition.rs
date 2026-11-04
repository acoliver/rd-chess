use super::board::{offset, opposite, pawn_step};
use super::{CastlingRights, Core, LegalMove, Man, PieceKind};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct BoardUndo {
    changes: [(u8, Option<Man>); 4],
    len: u8,
    kings: [u8; 2],
    rights: CastlingRights,
    ep: Option<u8>,
    ep_hash: u64,
}

pub(super) fn castling_rook(chess_move: LegalMove) -> Option<(u8, u8)> {
    if chess_move.from.abs_diff(chess_move.to) != 2 {
        return None;
    }
    let base = chess_move.from & 0x70;
    Some(if chess_move.to > chess_move.from {
        (base + 7, base + 5)
    } else {
        (base, base + 3)
    })
}

impl BoardUndo {
    fn remember(&mut self, core: &Core, location: u8) {
        self.changes[usize::from(self.len)] = (location, core.at(location));
        self.len += 1;
    }

    fn changes(&self) -> impl Iterator<Item = (u8, Option<Man>)> + '_ {
        self.changes[..usize::from(self.len)].iter().copied()
    }
}

impl Core {
    pub fn remember(&self, chess_move: LegalMove) -> BoardUndo {
        let mut saved = BoardUndo {
            changes: [(0, None); 4],
            len: 0,
            kings: self.kings,
            rights: self.rights,
            ep: self.ep,
            ep_hash: self.ep_hash(),
        };
        saved.remember(self, chess_move.from);
        saved.remember(self, chess_move.to);
        if self.is_en_passant(chess_move) {
            let victim = offset(chess_move.to, -pawn_step(self.side)).expect("ep victim on board");
            saved.remember(self, victim);
        } else if self.at(chess_move.from).expect("generated mover").kind == PieceKind::King
            && let Some((from, to)) = castling_rook(chess_move)
        {
            saved.remember(self, from);
            saved.remember(self, to);
        }
        saved
    }

    pub fn restore(&mut self, saved: BoardUndo) {
        for (location, man) in saved.changes() {
            self.board[usize::from(location)] = man;
        }
        self.kings = saved.kings;
        self.rights = saved.rights;
        self.ep = saved.ep;
        self.side = opposite(self.side);
    }

    pub fn hash_after(&self, previous: u64, saved: &BoardUndo) -> u64 {
        let mut hash = previous ^ super::hash::key(1536) ^ saved.ep_hash ^ self.ep_hash();
        for bit in 0..4 {
            if (self.rights.0 ^ saved.rights.0) & (1 << bit) != 0 {
                hash ^= super::hash::key(1537 + bit);
            }
        }
        for (location, man) in saved.changes() {
            hash ^= super::hash::piece_key(location, man);
            hash ^= super::hash::piece_key(location, self.at(location));
        }
        hash
    }
}
