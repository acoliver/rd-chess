use super::attacks::{KINGS, KNIGHTS, PAWNS, Targets};
use super::board::{KING_STEPS, offset, side_index};
use super::{Core, LegalMove, PieceKind};

pub(super) struct Safety {
    king: u8,
    pinned: u128,
    evasion: u128,
}

fn bit(location: u8) -> u128 {
    1_u128 << location
}

fn direction(from: u8, to: u8) -> i16 {
    let delta = i16::from(to) - i16::from(from);
    for step in [16, 17, 15] {
        if delta % step == 0 {
            return step * delta.signum();
        }
    }
    if from >> 4 == to >> 4 {
        delta.signum()
    } else {
        0
    }
}

impl Safety {
    pub fn new(core: &Core) -> Self {
        let king = core.kings[side_index(core.side)];
        let mut result = Self {
            king,
            pinned: 0,
            evasion: u128::MAX,
        };
        for step in KING_STEPS {
            result.ray(core, step);
        }
        result.leapers(core, &KNIGHTS[usize::from(king)], PieceKind::Knight);
        result.leapers(core, &KINGS[usize::from(king)], PieceKind::King);
        result.leapers(
            core,
            &PAWNS[side_index(core.side)][usize::from(king)],
            PieceKind::Pawn,
        );
        result
    }

    fn leapers(&mut self, core: &Core, targets: &Targets, kind: PieceKind) {
        for from in targets.iter() {
            if core
                .at(from)
                .is_some_and(|man| man.side != core.side && man.kind == kind)
            {
                self.evasion &= bit(from);
            }
        }
    }

    fn ray(&mut self, core: &Core, step: i16) {
        let mut cursor = offset(self.king, step);
        let mut blocker = None;
        let mut ray = 0;
        let slider = if step.abs() == 1 || step.abs() == 16 {
            PieceKind::Rook
        } else {
            PieceKind::Bishop
        };
        while let Some(to) = cursor {
            ray |= bit(to);
            if let Some(man) = core.at(to) {
                if man.side != core.side {
                    if man.kind == slider || man.kind == PieceKind::Queen {
                        if let Some(pinned) = blocker {
                            self.pinned |= bit(pinned);
                        } else {
                            self.evasion &= ray;
                        }
                    }
                    break;
                }
                if blocker.replace(to).is_some() {
                    break;
                }
            }
            cursor = offset(to, step);
        }
    }

    pub fn permits(&self, core: &Core, chess_move: LegalMove) -> bool {
        // King moves change the attack target; EP removes a second blocker.
        // These exceptional occupancies still use the complete safety test.
        if chess_move.from == self.king || core.is_en_passant(chess_move) {
            return !core.after(chess_move).in_check(core.side);
        }
        self.evasion & bit(chess_move.to) != 0
            && (self.pinned & bit(chess_move.from) == 0
                || direction(self.king, chess_move.from) == direction(self.king, chess_move.to))
    }
}
