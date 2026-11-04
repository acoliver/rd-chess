use super::board::{colour, name, piece};
use super::material::{line_reward, reward};
use super::{Fact, FactKind};
use crate::rules::{ChessState, LegalMove};

pub(super) fn sentence(state: &ChessState, played: LegalMove, fact: &Fact) -> String {
    let san = state.to_san(played).expect("validated move");
    match &fact.kind {
        FactKind::Capture {
            attacker,
            victim,
            square,
            ..
        } => format!(
            "{san} captures the {} on {} with a {}.",
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
        } => {
            let mut after = state.clone();
            after.play(played).expect("verified played move");
            format!(
                "{san} leaves the {} on {} hanging: the exchange wins {} for {}.",
                piece(*kind),
                name(*square),
                reward(&after, &fact.line[1..], *winner, *gain),
                colour(*winner)
            )
        }
        FactKind::Exchange { gain } => exchange(state, &san, fact, *gain),
        _ => motif(state, &san, fact),
    }
}

fn motif(state: &ChessState, san: &str, fact: &Fact) -> String {
    match &fact.kind {
        FactKind::Fork { targets, .. } => fork(state, san, fact, targets),
        FactKind::Pin {
            front,
            rear,
            rear_piece,
            ..
        } => format!(
            "{san} pins {} to the {} on {} and wins {} in this line.",
            name(*front),
            piece(*rear_piece),
            name(*rear),
            line_reward(state, &fact.line)
        ),
        FactKind::Skewer { front, rear, .. } => format!(
            "{san} skewers {} and {} and wins {} in this line.",
            name(*front),
            name(*rear),
            line_reward(state, &fact.line)
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
            "{san} uncovers the {} on {} against {} and wins {} in this line.",
            piece(*kind),
            name(*attacker),
            name(*target),
            line_reward(state, &fact.line)
        ),
        _ => opportunity(state, san, fact),
    }
}

fn opportunity(state: &ChessState, san: &str, fact: &Fact) -> String {
    match &fact.kind {
        FactKind::MateThreat => {
            format!("{san} creates a mate-in-one threat; this line ends in checkmate.")
        }
        FactKind::MaterialThreat { gain } => format!(
            "{san} threatens material next move and wins {} in this line.",
            reward(state, &fact.line, state.side_to_move(), *gain)
        ),
        FactKind::MissedMate { alternative } => format!(
            "{san} misses {}, which gives checkmate in this line.",
            state.to_san(*alternative).expect("verified alternative")
        ),
        FactKind::MissedMaterial { alternative, gain } => format!(
            "{san} misses {}; its line wins {}.",
            state.to_san(*alternative).expect("verified alternative"),
            reward(state, &fact.line, state.side_to_move(), *gain)
        ),
        _ => unreachable!("direct fact handled by sentence"),
    }
}

fn fork(state: &ChessState, san: &str, fact: &Fact, targets: &[crate::Square]) -> String {
    let mut after = state.clone();
    after.play(fact.line[0]).expect("verified fork move");
    let mut targets = targets.to_vec();
    targets.sort_by_key(|&sq| after.piece_at(sq).expect("fork target").1 != crate::PieceKind::King);
    let describe = |sq| {
        let kind = after.piece_at(sq).expect("fork target").1;
        format!("the {} on {}", piece(kind), name(sq))
    };
    format!(
        "{san} forks {} and wins {} in this line.",
        targets
            .into_iter()
            .map(describe)
            .collect::<Vec<_>>()
            .join(" and "),
        line_reward(state, &fact.line)
    )
}

fn exchange(state: &ChessState, san: &str, fact: &Fact, gain: i32) -> String {
    match gain.cmp(&0) {
        std::cmp::Ordering::Greater => format!(
            "{san} wins {} in the exchange.",
            reward(state, &fact.line, state.side_to_move(), gain)
        ),
        std::cmp::Ordering::Less => format!(
            "{san} loses {} in the exchange.",
            reward(
                state,
                &fact.line,
                if state.side_to_move() == crate::Side::Ivory {
                    crate::Side::Onyx
                } else {
                    crate::Side::Ivory
                },
                -gain
            )
        ),
        std::cmp::Ordering::Equal => format!("{san} breaks even in the exchange."),
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
