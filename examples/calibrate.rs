//! Run with --release; see README's opponent calibration section.
#![deny(clippy::too_many_lines, clippy::too_many_arguments)]

use chess3d::search::selfplay::{
    MatchConfig, MatchReport, MoveStats, opening_positions, play_match,
};
use chess3d::search::{LADDER, choose_move_measured};
use std::time::Instant;

fn main() -> Result<(), String> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.as_slice() == ["timing"] {
        timing(&opening_positions());
        return Ok(());
    }
    if args.as_slice() == ["stress"] {
        timing(&stress_positions());
        return Ok(());
    }
    let (pairs, offset) = match args.first().map(String::as_str) {
        Some("ladder") => (
            (1..8).map(|level| (level, level + 1)).collect::<Vec<_>>(),
            1,
        ),
        Some(_) if args.len() == 6 => (vec![(number(&args, 0)?, number(&args, 1)?)], 2),
        _ => return Err(usage().into()),
    };
    if args.len() != offset + 4 {
        return Err(usage().into());
    }
    let config = MatchConfig {
        games: number(&args, offset)?,
        threads: number(&args, offset + 1)?,
        max_plies: number(&args, offset + 2)?,
        seed: number(&args, offset + 3)?,
    };
    for (low, high) in pairs {
        if !(1..=8).contains(&low) || !(1..=8).contains(&high) || low == high {
            return Err("levels must be distinct and in 1..=8".into());
        }
        run(low, high, config)?;
    }
    Ok(())
}

fn usage() -> &'static str {
    "calibrate timing|stress | calibrate ladder GAMES THREADS MAX_PLIES SEED | calibrate LOW HIGH GAMES THREADS MAX_PLIES SEED"
}

fn number<T: std::str::FromStr>(args: &[String], index: usize) -> Result<T, String> {
    args.get(index)
        .ok_or_else(|| usage().to_string())?
        .parse()
        .map_err(|_| format!("invalid number at argument {}", index + 1))
}

fn run(low: usize, high: usize, config: MatchConfig) -> Result<(), String> {
    println!("MATCH {low}v{high} {config:?}");
    let start = Instant::now();
    let games = play_match([LADDER[low - 1], LADDER[high - 1]], config)?;
    for game in &games {
        let moves = game
            .moves
            .iter()
            .map(|chess_move| chess_move.to_uci())
            .collect::<Vec<_>>()
            .join(" ");
        println!("game={} finish={:?} moves={moves}", game.index, game.finish);
    }
    let report = MatchReport::new(&games);
    println!(
        "RESULT {low}v{high} W/D/L={}/{}/{} score={:.2}% CI95=[{:.2},{:.2}]% capped={} wall={:.3}s",
        report.wins,
        report.draws,
        report.losses,
        report.score * 100.0,
        report.interval[0] * 100.0,
        report.interval[1] * 100.0,
        report.capped,
        start.elapsed().as_secs_f64()
    );
    stats(low, report.stats[0]);
    stats(high, report.stats[1]);
    Ok(())
}

fn stats(level: usize, stats: MoveStats) {
    let moves = stats.moves.max(1);
    println!(
        "TIME level={level} moves={} avg_nodes={} avg_ms={:.3} max_ms={:.3}",
        stats.moves,
        stats.nodes / u64::from(moves),
        stats.elapsed.as_secs_f64() * 1000.0 / f64::from(moves),
        stats.longest.as_secs_f64() * 1000.0
    );
}

fn timing(positions: &[chess3d::rules::ChessState]) {
    for (level, profile) in LADDER.iter().enumerate() {
        let mut total = MoveStats::default();
        for (index, position) in positions.iter().enumerate() {
            let start = Instant::now();
            let decision = choose_move_measured(
                position,
                profile,
                u64::try_from(index).expect("opening index"),
            );
            let elapsed = start.elapsed();
            total.merge(MoveStats {
                moves: 1,
                nodes: decision.nodes,
                elapsed,
                longest: elapsed,
            });
        }
        stats(level + 1, total);
    }
}

fn stress_positions() -> Vec<chess3d::rules::ChessState> {
    [
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
        "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    ]
    .into_iter()
    .map(|fen| chess3d::rules::ChessState::from_fen(fen).expect("perft fixture is legal"))
    .collect()
}
