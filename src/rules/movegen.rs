use super::board::{
    DIAGONALS, KING_STEPS, KNIGHT_STEPS, ORTHOGONALS, home_rank, offset, pawn_step,
};
use super::{Core, LegalMove, Man, PieceKind, PromotionPiece};

impl Core {
    pub fn legal_moves(&self) -> Vec<LegalMove> {
        let mut candidates = Vec::with_capacity(64);
        for (from, man) in self.men().filter(|(_, man)| man.side == self.side) {
            self.piece_moves(from, man, &mut candidates);
        }
        candidates.retain(|&chess_move| !self.after(chess_move).in_check(self.side));
        candidates
    }

    fn piece_moves(&self, from: u8, man: Man, output: &mut Vec<LegalMove>) {
        match man.kind {
            PieceKind::Pawn => self.pawn_moves(from, output),
            PieceKind::Knight => self.ray_moves(from, &KNIGHT_STEPS, false, output),
            PieceKind::Bishop => self.ray_moves(from, &DIAGONALS, true, output),
            PieceKind::Rook => self.ray_moves(from, &ORTHOGONALS, true, output),
            PieceKind::Queen => self.ray_moves(from, &KING_STEPS, true, output),
            PieceKind::King => {
                self.ray_moves(from, &KING_STEPS, false, output);
                self.castles(from, output);
            }
        }
    }

    fn ray_moves(&self, from: u8, steps: &[i16], sliding: bool, output: &mut Vec<LegalMove>) {
        for &step in steps {
            let mut cursor = offset(from, step);
            while let Some(to) = cursor {
                let occupant = self.at(to);
                if occupant.is_some_and(|man| man.side == self.side || man.kind == PieceKind::King)
                {
                    break;
                }
                output.push(LegalMove {
                    from,
                    to,
                    promotion: None,
                });
                if !sliding || occupant.is_some() {
                    break;
                }
                cursor = offset(to, step);
            }
        }
    }

    fn pawn_moves(&self, from: u8, output: &mut Vec<LegalMove>) {
        let step = pawn_step(self.side);
        if let Some(to) = offset(from, step).filter(|&to| self.at(to).is_none()) {
            Self::pawn_arrival(from, to, output);
            let start_rank = if self.side == super::Side::Ivory {
                1
            } else {
                6
            };
            if from >> 4 == start_rank
                && let Some(double) = offset(to, step).filter(|&to| self.at(to).is_none())
            {
                output.push(LegalMove {
                    from,
                    to: double,
                    promotion: None,
                });
            }
        }
        for file in [-1, 1] {
            if let Some(to) = offset(from, step + file) {
                let capture = self
                    .at(to)
                    .is_some_and(|man| man.side != self.side && man.kind != PieceKind::King);
                if capture || self.ep == Some(to) {
                    Self::pawn_arrival(from, to, output);
                }
            }
        }
    }

    fn pawn_arrival(from: u8, to: u8, output: &mut Vec<LegalMove>) {
        if to >> 4 == 0 || to >> 4 == 7 {
            output.extend(PromotionPiece::ALL.map(|promotion| LegalMove {
                from,
                to,
                promotion: Some(promotion),
            }));
        } else {
            output.push(LegalMove {
                from,
                to,
                promotion: None,
            });
        }
    }

    fn castles(&self, from: u8, output: &mut Vec<LegalMove>) {
        if self.in_check(self.side) {
            return;
        }
        for king_side in [true, false] {
            if self.can_castle(king_side) {
                output.push(LegalMove {
                    from,
                    to: home_rank(self.side) * 16 + if king_side { 6 } else { 2 },
                    promotion: None,
                });
            }
        }
    }

    fn can_castle(&self, king_side: bool) -> bool {
        let entitled = if king_side {
            self.rights.king_side(self.side)
        } else {
            self.rights.queen_side(self.side)
        };
        if !entitled {
            return false;
        }
        let base = home_rank(self.side) * 16;
        let empty_files: &[u8] = if king_side { &[5, 6] } else { &[1, 2, 3] };
        if empty_files
            .iter()
            .any(|file| self.at(base + file).is_some())
        {
            return false;
        }
        let transit = LegalMove {
            from: base + 4,
            to: base + if king_side { 5 } else { 3 },
            promotion: None,
        };
        !self.after(transit).in_check(self.side)
    }

    pub fn legal_ep_available(&self) -> bool {
        let Some(target) = self.ep else {
            return false;
        };
        [-1, 1]
            .iter()
            .filter_map(|file| offset(target, -pawn_step(self.side) + file))
            .any(|from| {
                self.at(from)
                    .is_some_and(|man| man.side == self.side && man.kind == PieceKind::Pawn)
                    && !self
                        .after(LegalMove {
                            from,
                            to: target,
                            promotion: None,
                        })
                        .in_check(self.side)
            })
    }
}
