use super::fen::{kind_letter, square_name};
use super::{ChessState, LegalMove, MoveError, PieceKind};

impl LegalMove {
    pub fn to_uci(self) -> String {
        let mut result = square_name(self.from);
        result.push_str(&square_name(self.to));
        if let Some(promotion) = self.promotion {
            result.push(kind_letter(promotion.kind()).to_ascii_lowercase());
        }
        result
    }
}

impl ChessState {
    /// Strict long algebraic UCI, with a lower-case promotion suffix when required.
    pub fn parse_uci(&self, text: &str) -> Result<LegalMove, MoveError> {
        self.legal_moves()
            .into_iter()
            .find(|chess_move| chess_move.to_uci() == text)
            .ok_or(MoveError::Notation)
    }

    /// Strict SAN, including necessary disambiguation and exact check/mate suffix.
    pub fn parse_san(&self, text: &str) -> Result<LegalMove, MoveError> {
        let legal = self.legal_moves();
        legal
            .iter()
            .copied()
            .find(|&chess_move| self.san_for(chess_move, &legal) == text)
            .ok_or(MoveError::Notation)
    }

    pub fn to_san(&self, chess_move: LegalMove) -> Result<String, MoveError> {
        let legal = self.legal_moves();
        if !legal.contains(&chess_move) {
            return Err(MoveError::IllegalMove);
        }
        Ok(self.san_for(chess_move, &legal))
    }

    fn san_for(&self, chess_move: LegalMove, legal: &[LegalMove]) -> String {
        let man = self.core.at(chess_move.from).expect("legal mover exists");
        let mut text =
            if man.kind == PieceKind::King && chess_move.from.abs_diff(chess_move.to) == 2 {
                if chess_move.to > chess_move.from {
                    "O-O".into()
                } else {
                    "O-O-O".into()
                }
            } else {
                self.san_body(chess_move, man.kind, legal)
            };
        let next = self.core.after(chess_move);
        if next.in_check(next.side) {
            text.push(if next.legal_moves().is_empty() {
                '#'
            } else {
                '+'
            });
        }
        text
    }

    fn san_body(&self, chess_move: LegalMove, kind: PieceKind, legal: &[LegalMove]) -> String {
        let mut text = String::new();
        let capture = self.core.is_capture(chess_move);
        if kind == PieceKind::Pawn {
            if capture {
                text.push(char::from(b'a' + chess_move.from().file));
            }
        } else {
            text.push(kind_letter(kind));
            text.push_str(&self.disambiguation(chess_move, kind, legal));
        }
        if capture {
            text.push('x');
        }
        text.push_str(&square_name(chess_move.to));
        if let Some(promotion) = chess_move.promotion {
            text.push('=');
            text.push(kind_letter(promotion.kind()));
        }
        text
    }

    fn disambiguation(
        &self,
        chess_move: LegalMove,
        kind: PieceKind,
        legal: &[LegalMove],
    ) -> String {
        let rivals: Vec<_> = legal
            .iter()
            .filter(|other| {
                other.to == chess_move.to
                    && other.from != chess_move.from
                    && self.core.at(other.from).is_some_and(|man| man.kind == kind)
            })
            .collect();
        if rivals.is_empty() {
            return String::new();
        }
        let from = chess_move.from();
        let same_file = rivals.iter().any(|other| other.from().file == from.file);
        let same_rank = rivals.iter().any(|other| other.from().rank == from.rank);
        if !same_file {
            return char::from(b'a' + from.file).to_string();
        }
        if !same_rank {
            return char::from(b'1' + from.rank).to_string();
        }
        square_name(chess_move.from)
    }
}
