use super::{ChessState, LegalMove, MoveError, PieceKind, Side, opposite, square};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DrawClaims {
    pub fifty_move: bool,
    pub threefold: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DrawReason {
    Stalemate,
    InsufficientMaterial,
    SeventyFiveMove,
    FivefoldRepetition,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GameStatus {
    Ongoing { in_check: bool, claims: DrawClaims },
    Checkmate { winner: Side },
    Draw(DrawReason),
}

impl ChessState {
    pub fn repetition_count(&self) -> usize {
        let current = self.position_hash();
        self.repetition
            .iter()
            .filter(|&&hash| hash == current)
            .count()
    }

    /// Entitlements from counters/history, separate from automatic outcomes.
    /// Only `Ongoing` positions permit a claim; mate always takes precedence.
    pub fn draw_claims(&self) -> DrawClaims {
        DrawClaims {
            fifty_move: self.halfmove >= 100,
            threefold: self.repetition_count() >= 3,
        }
    }

    /// Evaluate the declared intended move without committing it to this state.
    pub fn claims_after(&self, chess_move: LegalMove) -> Result<DrawClaims, MoveError> {
        let mut next = self.clone();
        next.play(chess_move)?;
        Ok(next.draw_claims())
    }

    pub fn status(&self) -> GameStatus {
        if self.legal_moves().is_empty() {
            return if self.in_check() {
                GameStatus::Checkmate {
                    winner: opposite(self.core.side),
                }
            } else {
                GameStatus::Draw(DrawReason::Stalemate)
            };
        }
        if self.insufficient_material() {
            return GameStatus::Draw(DrawReason::InsufficientMaterial);
        }
        if self.halfmove >= 150 {
            return GameStatus::Draw(DrawReason::SeventyFiveMove);
        }
        if self.repetition_count() >= 5 {
            return GameStatus::Draw(DrawReason::FivefoldRepetition);
        }
        GameStatus::Ongoing {
            in_check: self.in_check(),
            claims: self.draw_claims(),
        }
    }

    fn insufficient_material(&self) -> bool {
        let minors: Vec<_> = self
            .core
            .men()
            .filter(|(_, man)| man.kind != PieceKind::King)
            .collect();
        if minors.is_empty() {
            return true;
        }
        if minors.len() == 1 {
            return matches!(minors[0].1.kind, PieceKind::Knight | PieceKind::Bishop);
        }
        let colour = square(minors[0].0).is_dark();
        minors.iter().all(|(location, man)| {
            man.kind == PieceKind::Bishop && square(*location).is_dark() == colour
        })
    }
}
