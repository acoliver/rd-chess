use crate::{
    Side,
    rules::{ChessState, DrawReason, GameStatus},
};

pub(super) fn status_text(game: &ChessState) -> String {
    let side = side_name(game.side_to_move());
    match game.status() {
        GameStatus::Ongoing { in_check, claims } => {
            let mut text = format!("{side} to move");
            if in_check {
                text.push_str(" (check)");
            }
            if claims.fifty_move {
                text.push_str("\nfifty-move rule claim available");
            }
            if claims.threefold {
                text.push_str("\nthreefold repetition claim available");
            }
            text
        }
        GameStatus::Checkmate { winner } => format!("Checkmate: {} wins", side_name(winner)),
        GameStatus::Draw(reason) => format!(
            "Draw: {}",
            match reason {
                DrawReason::Stalemate => "stalemate",
                DrawReason::InsufficientMaterial => "insufficient material",
                DrawReason::SeventyFiveMove => "seventy-five-move rule",
                DrawReason::FivefoldRepetition => "fivefold repetition",
            }
        ),
    }
}
pub(super) fn side_name(side: Side) -> &'static str {
    match side {
        Side::Ivory => "White",
        Side::Onyx => "Black",
    }
}
