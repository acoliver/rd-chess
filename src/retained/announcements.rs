use crate::{
    PieceKind, Side, Square,
    rules::{ChessState, GameStatus, LegalMove, PromotionPiece},
};
use std::fmt::Write;

pub(super) fn square_name(square: Square) -> String {
    format!("{}{}", char::from(b'a' + square.file), square.rank + 1)
}
pub(super) fn piece_name(kind: PieceKind) -> &'static str {
    match kind {
        PieceKind::Pawn => "pawn",
        PieceKind::Knight => "knight",
        PieceKind::Bishop => "bishop",
        PieceKind::Rook => "rook",
        PieceKind::Queen => "queen",
        PieceKind::King => "king",
    }
}
pub(super) fn square_label(game: &ChessState, square: Square) -> String {
    let occupant = game.piece_at(square).map_or_else(
        || "empty".into(),
        |(side, kind)| {
            format!(
                "{} {}",
                super::status::side_name(side).to_lowercase(),
                piece_name(kind)
            )
        },
    );
    format!("{}, {occupant}", square_name(square))
}
pub(super) fn move_words(game: &ChessState, chess_move: LegalMove) -> String {
    let (side, kind) = game.piece_at(chess_move.from()).expect("legal mover");
    let name = super::status::side_name(side);
    let from = square_name(chess_move.from());
    let to = square_name(chess_move.to());
    if kind == PieceKind::King && chess_move.from().file.abs_diff(chess_move.to().file) == 2 {
        let (wing, rook_from, rook_to) = if chess_move.to().file == 6 {
            ("kingside", 'h', 'f')
        } else {
            ("queenside", 'a', 'd')
        };
        let rank = chess_move.from().rank + 1;
        return format!(
            "{name} castles {wing}, king from {from} to {to}, rook from {rook_from}{rank} to {rook_to}{rank}"
        );
    }
    let mut words = format!("{name} {} from {from} to {to}", piece_name(kind));
    capture_words(&mut words, game, chess_move, kind);
    if let Some(promotion) = chess_move.promotion() {
        write!(words, ", promotes to {}", promotion_name(promotion)).expect("string formatting");
    }
    words
}
fn capture_words(words: &mut String, game: &ChessState, chess_move: LegalMove, kind: PieceKind) {
    let en_passant = kind == PieceKind::Pawn
        && chess_move.from().file != chess_move.to().file
        && game.piece_at(chess_move.to()).is_none();
    let square = if en_passant {
        Square {
            file: chess_move.to().file,
            rank: chess_move.from().rank,
        }
    } else {
        chess_move.to()
    };
    if let Some((side, kind)) = game.piece_at(square) {
        write!(
            words,
            ", captures {} {}",
            super::status::side_name(side).to_lowercase(),
            piece_name(kind)
        )
        .expect("string formatting");
        if en_passant {
            words.push_str(" en passant");
        }
    }
}
fn promotion_name(piece: PromotionPiece) -> &'static str {
    match piece {
        PromotionPiece::Queen => "queen",
        PromotionPiece::Rook => "rook",
        PromotionPiece::Bishop => "bishop",
        PromotionPiece::Knight => "knight",
    }
}
pub(super) fn outcome(words: &mut String, game: &ChessState) {
    match game.status() {
        GameStatus::Checkmate { winner } => {
            write!(
                words,
                ", checkmate. {} wins",
                super::status::side_name(winner)
            )
            .expect("string formatting");
        }
        GameStatus::Draw(_) => {
            write!(words, ". {}", super::status::status_text(game)).expect("string formatting");
        }
        GameStatus::Ongoing { in_check: true, .. } => words.push_str(", check"),
        GameStatus::Ongoing { .. } => {}
    }
}
pub(super) fn oriented_square(index: usize, side: Side) -> Square {
    let file = u8::try_from(index % 8).expect("eight files");
    let row = u8::try_from(index / 8).expect("eight ranks");
    if side == Side::Ivory {
        Square {
            file,
            rank: 7 - row,
        }
    } else {
        Square {
            file: 7 - file,
            rank: row,
        }
    }
}
