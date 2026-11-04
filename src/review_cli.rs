use chess3d::Side;
use chess3d::review::{PlyReview, Review, ReviewConfig, ReviewStep};
use chess3d::rules::{ChessState, INITIAL_FEN};
use chess3d::search::Score;
use std::error::Error;
use std::sync::atomic::AtomicBool;

pub fn run(text: &str, fen: Option<&str>) -> Result<(), Box<dyn Error>> {
    let initial = ChessState::from_fen(fen.unwrap_or(INITIAL_FEN))
        .map_err(|error| format!("invalid review FEN: {error}"))?;
    let player = initial.side_to_move();
    let mut state = initial.clone();
    let mut moves = Vec::new();
    for uci in text.split_whitespace() {
        let chess_move = state
            .parse_uci(uci)
            .map_err(|error| format!("invalid review UCI move '{uci}': {error}"))?;
        state.play(chess_move)?;
        moves.push(chess_move);
    }
    let mut review = Review::new(initial, moves, player, ReviewConfig::default())?;
    let cancel = AtomicBool::new(false);
    while review.advance(&cancel)? == ReviewStep::Advanced {}
    print_report(&review, player)?;
    Ok(())
}

fn print_report(review: &Review, player: Side) -> Result<(), Box<dyn Error>> {
    let report = review.report();
    let progress = review.progress();
    println!(
        "Neutral review v{}: {}/{} plies; {} nodes/search; moments for {}",
        report.analysis_version,
        progress.completed,
        progress.total,
        report.nodes_per_search,
        side_name(player)
    );
    println!(
        "Evaluations are from the mover's perspective. Search estimates, not proofs of move quality."
    );
    println!(
        "ply | played | best | before | after | loss | label | depth best/played | best PV (SAN)"
    );
    for row in &report.plies {
        print_row(row);
    }
    println!("Teachable moments ({}):", report.moments.len());
    for moment in &report.moments {
        println!(
            "ply {}: {} ({}, {}); try {}",
            moment.ply,
            moment.position_before.to_san(moment.played)?,
            moment.label,
            moment.loss,
            moment.position_before.to_san(moment.better)?
        );
        println!("  Before: {}", moment.position_before.to_fen());
        println!("  Show the reply: {}", moment.refutation.san.join(" "));
    }
    Ok(())
}

fn print_row(row: &PlyReview) {
    let mover = row.position_before.side_to_move();
    let best = row.best_line.san.first().map_or("-", String::as_str);
    println!(
        "{} | {} | {} | {} | {} | {} | {} | {}/{} | {}",
        row.ply,
        row.played_san,
        best,
        score_text(row.before, mover),
        score_text(row.after, mover),
        row.loss,
        row.label,
        row.best_depth,
        row.played_depth,
        row.best_line.san.join(" ")
    );
    if let Some(reason) = row.uncertainty {
        println!("  Unclear: {reason:?}");
    }
    if !row.refutation.san.is_empty() {
        println!(
            "  After played move, best reply: {}",
            row.refutation.san.join(" ")
        );
    }
}

fn score_text(score: Score, mover: Side) -> String {
    match score {
        Score::Centipawns(cp) => format!("{cp:+} cp"),
        Score::Mate { winner, plies } => format!(
            "{}mate/{plies} ply",
            if winner == mover { "+" } else { "-" }
        ),
    }
}

fn side_name(side: Side) -> &'static str {
    match side {
        Side::Ivory => "white",
        Side::Onyx => "black",
    }
}
