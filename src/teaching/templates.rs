use super::board::{colour, name, piece};
use super::{Fact, FactKind};
use crate::rules::{ChessState, LegalMove};

pub(super) fn sentence(state: &ChessState, played: LegalMove, fact: &Fact) -> String {
    let san = state.to_san(played).expect("validated move");
    match &fact.kind {
        FactKind::Capture {
            attacker,
            victim,
            square,
            balance_change,
        } => format!(
            "{san} captures the {} on {} with a {}; material balance changes by {balance_change:+} centipawns.",
            piece(*victim),
            name(*square),
            piece(*attacker)
        ),
        FactKind::Check => format!("{san} gives check."),
        FactKind::Checkmate { .. } => format!("{san} is checkmate."),
        FactKind::Promotion { into } => format!("{san} promotes the pawn to a {}.", piece(*into)),
        FactKind::Castling { king_side } => format!("{san} castles {}.", wing(*king_side)),
        FactKind::CastlingRightsLost {
            side,
            king_side,
            queen_side,
        } => format!(
            "{san} loses {}'s {} castling rights.",
            colour(*side),
            rights(*king_side, *queen_side)
        ),
        FactKind::HangingPiece {
            piece: kind,
            square,
            winner,
            gain,
        } => format!(
            "{san} leaves the {} on {} hanging: the verified exchange wins {gain} centipawns for {}.",
            piece(*kind),
            name(*square),
            colour(*winner)
        ),
        FactKind::Exchange { gain } => exchange(&san, *gain),
        _ => motif(state, &san, fact),
    }
}

fn motif(state: &ChessState, san: &str, fact: &Fact) -> String {
    match &fact.kind {
        FactKind::Fork { targets, .. } => format!(
            "{san} forks {}; the verified line wins material.",
            targets
                .iter()
                .map(|&sq| name(sq))
                .collect::<Vec<_>>()
                .join(" and ")
        ),
        FactKind::Pin {
            front,
            rear,
            rear_piece,
            ..
        } => format!(
            "{san} pins {} to the {} on {}; the verified line wins material.",
            name(*front),
            piece(*rear_piece),
            name(*rear)
        ),
        FactKind::Skewer { front, rear, .. } => format!(
            "{san} skewers {} and {}; the verified line wins material.",
            name(*front),
            name(*rear)
        ),
        FactKind::DiscoveredCheck {
            attacker,
            piece: kind,
        } => format!(
            "{san} uncovers check from the {} on {}.",
            piece(*kind),
            name(*attacker)
        ),
        FactKind::DiscoveredAttack {
            attacker,
            target,
            piece: kind,
        } => format!(
            "{san} uncovers the {} on {} against {}; the verified line wins material.",
            piece(*kind),
            name(*attacker),
            name(*target)
        ),
        FactKind::MateThreat => {
            format!("{san} creates a mate-in-one threat; the verified line ends in checkmate.")
        }
        FactKind::MaterialThreat { gain } => {
            format!("{san} threatens material next move; the verified line wins {gain} centipawns.")
        }
        FactKind::MissedMate { alternative } => format!(
            "{san} misses {}, which gives checkmate in the verified line.",
            state.to_san(*alternative).expect("verified alternative")
        ),
        FactKind::MissedMaterial { alternative, gain } => format!(
            "{san} misses {}; its verified line wins {gain} centipawns.",
            state.to_san(*alternative).expect("verified alternative")
        ),
        _ => unreachable!("direct fact handled by sentence"),
    }
}

fn exchange(san: &str, gain: i32) -> String {
    match gain.cmp(&0) {
        std::cmp::Ordering::Greater => {
            format!("{san} wins {gain} centipawns in the verified exchange.")
        }
        std::cmp::Ordering::Less => {
            format!("{san} loses {} centipawns in the verified exchange.", -gain)
        }
        std::cmp::Ordering::Equal => format!("{san} breaks even in the verified exchange."),
    }
}

fn wing(king_side: bool) -> &'static str {
    if king_side { "kingside" } else { "queenside" }
}

fn rights(king_side: bool, queen_side: bool) -> &'static str {
    match (king_side, queen_side) {
        (true, true) => "kingside and queenside",
        (true, false) => "kingside",
        (false, true) => "queenside",
        (false, false) => unreachable!("a lost right exists"),
    }
}

/// Formats a verified line in its actual succession of positions, not all moves
/// against the initial board. Invalid evidence is an internal invariant failure.
pub fn san_line(state: &ChessState, line: &[LegalMove]) -> String {
    let mut cursor = state.clone();
    line.iter()
        .map(|&mv| {
            let text = cursor.to_san(mv).expect("verified continuation");
            cursor.play(mv).expect("verified continuation");
            text
        })
        .collect::<Vec<_>>()
        .join(" ")
}
