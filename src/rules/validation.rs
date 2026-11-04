use super::board::{home_rank, offset, pawn_step, side_index};
use super::{ChessState, FenError, PieceKind, Side, opposite, square};

impl ChessState {
    pub(super) fn validate(&mut self) -> Result<(), FenError> {
        for side in [Side::Ivory, Side::Onyx] {
            self.validate_king(side)?;
            self.validate_material(side)?;
            self.validate_castling(side)?;
        }
        let white = square(self.core.kings[0]);
        let black = square(self.core.kings[1]);
        if white.file.abs_diff(black.file) <= 1 && white.rank.abs_diff(black.rank) <= 1 {
            return Err(FenError::AdjacentKings);
        }
        if self.core.in_check(opposite(self.core.side)) {
            return Err(FenError::PreviousSideInCheck);
        }
        self.validate_ep()
    }

    fn validate_king(&mut self, side: Side) -> Result<(), FenError> {
        let kings: Vec<_> = self
            .core
            .men()
            .filter(|(_, man)| man.side == side && man.kind == PieceKind::King)
            .map(|(location, _)| location)
            .collect();
        let [king] = kings.as_slice() else {
            return Err(FenError::Kings);
        };
        self.core.kings[side_index(side)] = *king;
        Ok(())
    }

    fn validate_material(&self, side: Side) -> Result<(), FenError> {
        let mut counts = [0_u8; 7];
        for (location, man) in self.core.men().filter(|(_, man)| man.side == side) {
            let slot = match man.kind {
                PieceKind::Pawn => 0,
                PieceKind::Knight => 1,
                PieceKind::Bishop => {
                    if square(location).is_dark() {
                        2
                    } else {
                        3
                    }
                }
                PieceKind::Rook => 4,
                PieceKind::Queen => 5,
                PieceKind::King => 6,
            };
            counts[slot] += 1;
            if man.kind == PieceKind::Pawn && [0, 7].contains(&(location >> 4)) {
                return Err(FenError::PawnRank);
            }
        }
        if counts[0] > 8 || counts.iter().sum::<u8>() > 16 {
            return Err(FenError::Material);
        }
        let promotions: u8 = counts
            .iter()
            .zip([8, 2, 1, 1, 2, 1, 1])
            .map(|(&count, original)| count.saturating_sub(original))
            .sum();
        if promotions > 8 - counts[0] {
            return Err(FenError::Material);
        }
        Ok(())
    }

    fn validate_castling(&self, side: Side) -> Result<(), FenError> {
        let base = home_rank(side) * 16;
        for (has_right, rook_file) in [
            (self.core.rights.king_side(side), 7),
            (self.core.rights.queen_side(side), 0),
        ] {
            if has_right {
                let king_ok = self
                    .core
                    .at(base + 4)
                    .is_some_and(|man| man.side == side && man.kind == PieceKind::King);
                let rook_ok = self
                    .core
                    .at(base + rook_file)
                    .is_some_and(|man| man.side == side && man.kind == PieceKind::Rook);
                if !king_ok || !rook_ok {
                    return Err(FenError::Castling);
                }
            }
        }
        Ok(())
    }

    fn validate_ep(&self) -> Result<(), FenError> {
        let Some(target) = self.core.ep else {
            return Ok(());
        };
        let expected_rank = if self.core.side == Side::Ivory { 5 } else { 2 };
        if target >> 4 != expected_rank || self.halfmove != 0 || self.core.at(target).is_some() {
            return Err(FenError::EnPassant);
        }
        let step = pawn_step(self.core.side);
        let victim = offset(target, -step).ok_or(FenError::EnPassant)?;
        let origin = offset(target, step).ok_or(FenError::EnPassant)?;
        let pawn_ok = self
            .core
            .at(victim)
            .is_some_and(|man| man.side == opposite(self.core.side) && man.kind == PieceKind::Pawn);
        if !pawn_ok || self.core.at(origin).is_some() {
            return Err(FenError::EnPassant);
        }
        self.validate_previous_double_push(victim, origin)
    }

    fn validate_previous_double_push(&self, victim: u8, origin: u8) -> Result<(), FenError> {
        if self.core.side == Side::Ivory && self.fullmove == 1 {
            return Err(FenError::EnPassant);
        }
        let mut previous = self.core;
        previous.board[usize::from(origin)] = previous.board[usize::from(victim)].take();
        // The current player could not already have been in check before the
        // opponent's double push. The previous mover's final safety is checked
        // by validate(), as for every other imported position.
        if previous.in_check(self.core.side) {
            return Err(FenError::EnPassant);
        }
        Ok(())
    }
}
