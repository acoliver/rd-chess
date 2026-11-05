use super::{Limits, PlyReview, ReviewLine, Score, Search, TABLE_SLOTS, classify};
use crate::rules::{ChessState, GameStatus, LegalMove, MoveError};
use crate::search::Analysis;
use std::sync::atomic::AtomicBool;

pub(super) fn analyze_ply(
    state: &ChessState,
    played: LegalMove,
    limits: Limits,
    index: usize,
) -> Result<PlyReview, MoveError> {
    let cancel = AtomicBool::new(false);
    let best = Search::new(TABLE_SLOTS).analyze(state, limits, &cancel);
    let same_move = best.completed_depth > 0 && best.pv.first() == Some(&played);
    let chosen = if same_move {
        best.clone()
    } else {
        Search::new(TABLE_SLOTS).analyze_move(
            state,
            played,
            Limits {
                depth: Some(best.completed_depth),
                ..limits
            },
            &cancel,
        )?
    };
    let loss = if same_move {
        super::Loss::Centipawns(0)
    } else {
        classify::loss(best.score, chosen.score, state.side_to_move())
    };
    let (label, uncertainty) = classify::label(&best, &chosen, loss, same_move);
    let best_line = line(state, &best)?;
    let played_line = line(state, &chosen)?;
    let refutation = ReviewLine {
        moves: played_line.moves.into_iter().skip(1).collect(),
        san: played_line.san.into_iter().skip(1).collect(),
    };
    Ok(PlyReview {
        ply: index + 1,
        position_before: state.clone(),
        played,
        played_san: state.to_san(played)?,
        best_move: best_line.moves.first().copied(),
        best_line,
        before: best.score,
        after: after_score(chosen.score),
        loss,
        label,
        uncertainty,
        best_depth: best.completed_depth,
        played_depth: chosen.completed_depth,
        best_nodes: best.nodes,
        played_nodes: if same_move { 0 } else { chosen.nodes },
        refutation,
    })
}

fn after_score(score: Score) -> Score {
    match score {
        Score::Mate { winner, plies } => Score::Mate {
            winner,
            plies: plies.saturating_sub(1),
        },
        cp @ Score::Centipawns(_) => cp,
    }
}

fn line(state: &ChessState, analysis: &Analysis) -> Result<ReviewLine, MoveError> {
    // Depth-zero PVs are unsearched fallbacks, not evidence of a continuation.
    let moves = if analysis.completed_depth == 0 {
        Vec::new()
    } else {
        analysis.pv.clone()
    };
    let mut position = state.clone();
    let mut san = Vec::with_capacity(moves.len());
    for &chess_move in &moves {
        san.push(position.to_san(chess_move)?);
        position.play(chess_move)?;
    }
    if analysis.completed_depth > 0
        && let Score::Mate { winner, plies } = analysis.score
    {
        assert_eq!(
            position.status(),
            GameStatus::Checkmate { winner },
            "search mate line must demonstrate mate"
        );
        assert_eq!(
            moves.len(),
            usize::from(plies),
            "mate distance must match the line"
        );
    }
    Ok(ReviewLine { moves, san })
}
