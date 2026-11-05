#[test]
fn black_evaluations_stay_relative_to_the_mover_after_play() {
    let report = finish(job("3qk3/8/8/R7/8/8/8/4K3 b - - 0 1", "d8d5", config()));
    let row = &report.plies[0];
    assert_eq!(row.label, Label::Blunder);
    assert!(matches!(row.before, Score::Centipawns(cp) if cp > 0));
    assert!(matches!(row.after, Score::Centipawns(cp) if cp < 0));
    assert_eq!(row.refutation.moves[0].to_uci(), "a5d5");
    assert!(report.moments.is_empty());
}

#[test]
fn available_claim_is_unclear_without_a_fabricated_best_line() {
    let report = finish(job("4k3/8/8/8/8/8/8/R3K3 w - - 100 80", "a1a2", config()));
    assert_eq!(report.plies[0].label, Label::Unclear);
    assert_eq!(report.plies[0].best_move, None);
    assert!(report.plies[0].best_line.moves.is_empty());
    assert!(report.moments.is_empty());
}

#[test]
fn record_cannot_continue_after_an_automatic_draw() {
    let state = ChessState::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    let chess_move = state.parse_uci("e1e2").unwrap();
    assert!(Review::new(state, vec![chess_move], Side::Ivory, config()).is_err());
}

use super::*;
use crate::rules::{GameStatus, INITIAL_FEN};
use std::sync::atomic::Ordering;

const QUEEN: &str = "4k3/8/8/8/r7/8/8/3QK3 w - - 0 1";
const MATE: &str = "7k/8/5KQ1/8/8/8/8/8 w - - 0 1";

fn config() -> ReviewConfig {
    ReviewConfig {
        limits: Limits {
            nodes: 5_000,
            depth: Some(2),
        },
        ..ReviewConfig::default()
    }
}

fn job(fen: &str, text: &str, config: ReviewConfig) -> Review {
    let initial = ChessState::from_fen(fen).unwrap();
    let mut game = initial.clone();
    let moves = text
        .split_whitespace()
        .map(|uci| {
            let chess_move = game.parse_uci(uci).unwrap();
            game.play(chess_move).unwrap();
            chess_move
        })
        .collect();
    Review::new(initial, moves, Side::Ivory, config).unwrap()
}

fn finish(mut review: Review) -> ReviewReport {
    let cancel = AtomicBool::new(false);
    while review.advance(&cancel).unwrap() == ReviewStep::Advanced {}
    review.report().clone()
}

#[test]
fn queen_blunder_includes_the_capturing_refutation() {
    let report = finish(job(QUEEN, "d1d4", config()));
    let row = &report.plies[0];
    assert_eq!(row.label, Label::Blunder);
    assert!(matches!(row.loss, Loss::Centipawns(cp) if cp > 300));
    assert_eq!(row.refutation.moves[0].to_uci(), "a4d4");
    assert_eq!(row.refutation.san[0], "Rxd4");
    assert_eq!(report.moments.len(), 1);
    assert_eq!(report.moments[0].position_before.to_fen(), QUEEN);
}

#[test]
fn missing_mate_in_one_is_named_without_centipawn_conversion() {
    let report = finish(job(MATE, "g6g8", config()));
    let row = &report.plies[0];
    assert_eq!(row.label, Label::MissedMate);
    assert_eq!(row.loss, Loss::MissedMate);
    assert_eq!(
        row.before,
        Score::Mate {
            winner: Side::Ivory,
            plies: 1
        }
    );
    assert_eq!(row.refutation.san[0], "Kxg8");
}

#[test]
fn forced_move_is_best() {
    let report = finish(job("r7/8/8/8/8/2k5/8/K7 w - - 0 1", "a1b1", config()));
    assert_eq!(report.plies[0].label, Label::Best);
    assert_eq!(report.plies[0].loss, Loss::Centipawns(0));
}

#[test]
fn quiet_equal_material_game_has_no_confident_mistakes() {
    let report = finish(job(INITIAL_FEN, "g1f3 g8f6 b1c3 b8c6", config()));
    assert!(report.moments.is_empty());
    assert!(report.plies.iter().all(|row| !row.label.is_mistake()));
}

#[test]
fn shallow_search_is_unclear_even_for_an_obvious_blunder() {
    let config = ReviewConfig {
        limits: Limits {
            nodes: 1,
            depth: None,
        },
        ..config()
    };
    let report = finish(job(QUEEN, "d1d4", config));
    assert_eq!(report.plies[0].label, Label::Unclear);
    assert!(report.moments.is_empty());
}

#[test]
fn moments_are_player_only_capped_and_ordered_by_loss() {
    let config = ReviewConfig {
        max_moments: 2,
        ..config()
    };
    let report = finish(job(QUEEN, "d1d4 a4a2 d4d2 a2a4 d2d4", config));
    assert_eq!(report.moments.len(), 2, "{report:?}");
    assert!(
        report
            .moments
            .iter()
            .all(|moment| moment.position_before.side_to_move() == Side::Ivory)
    );
    assert!(
        report
            .moments
            .windows(2)
            .all(|pair| pair[0].loss >= pair[1].loss)
    );
    let none = finish(job(
        QUEEN,
        "d1d4",
        ReviewConfig {
            max_moments: 0,
            ..config
        },
    ));
    assert!(none.moments.is_empty());
}

#[test]
fn deterministic_reports_include_identical_lines_and_limits() {
    let first = finish(job(QUEEN, "d1d4 a4d4", config()));
    assert_eq!(first, finish(job(QUEEN, "d1d4 a4d4", config())));
    assert_eq!(first.analysis_version, ANALYSIS_VERSION);
    assert_eq!(first.nodes_per_search, 5_000);
}

#[test]
fn cancellation_between_plies_resumes_without_changing_the_report() {
    let mut review = job(QUEEN, "d1d4 a4d4", config());
    let cancel = AtomicBool::new(true);
    assert_eq!(review.advance(&cancel).unwrap(), ReviewStep::Cancelled);
    assert_eq!(
        review.progress(),
        Progress {
            completed: 0,
            total: 2
        }
    );
    cancel.store(false, Ordering::Relaxed);
    assert_eq!(review.advance(&cancel).unwrap(), ReviewStep::Advanced);
    assert_eq!(review.progress().completed, 1);
    cancel.store(true, Ordering::Relaxed);
    let partial = review.report().clone();
    assert_eq!(review.advance(&cancel).unwrap(), ReviewStep::Cancelled);
    assert_eq!(review.report(), &partial);
    assert_eq!(finish(review), finish(job(QUEEN, "d1d4 a4d4", config())));
}

#[test]
fn every_analysis_and_refutation_line_replays_legally_with_exact_san() {
    let report = finish(job(QUEEN, "d1d4 a4d4", config()));
    for row in &report.plies {
        check_line(row.position_before.clone(), &row.best_line);
        let mut after = row.position_before.clone();
        after.play(row.played).unwrap();
        check_line(after, &row.refutation);
    }
}

fn check_line(mut state: ChessState, line: &ReviewLine) {
    assert_eq!(line.moves.len(), line.san.len());
    for (&chess_move, san) in line.moves.iter().zip(&line.san) {
        assert_eq!(&state.to_san(chess_move).unwrap(), san);
        state.play(chess_move).unwrap();
    }
}

#[test]
fn input_is_validated_before_any_analysis() {
    let initial = ChessState::initial();
    let chess_move = initial.parse_uci("e2e4").unwrap();
    assert!(Review::new(initial, vec![chess_move, chess_move], Side::Ivory, config()).is_err());
    let report = finish(job(MATE, "g6g7", config()));
    let mut after = report.plies[0].position_before.clone();
    after.play(report.plies[0].played).unwrap();
    assert!(matches!(after.status(), GameStatus::Checkmate { .. }));
    assert_eq!(
        report.plies[0].after,
        Score::Mate {
            winner: Side::Ivory,
            plies: 0
        }
    );
}

#[test]
fn loss_margin_and_threshold_boundaries_are_explicitly_unclear() {
    for cp in [0, 10, 20, 45, 50, 60, 140, 150, 160, 290, 300, 310] {
        assert_eq!(classify::centipawns(cp), Label::Unclear, "{cp}");
    }
    for (cp, label) in [
        (25, Label::Good),
        (90, Label::Inaccuracy),
        (200, Label::Mistake),
        (400, Label::Blunder),
    ] {
        assert_eq!(classify::centipawns(cp), label);
    }
}

#[test]
fn allowed_mate_uses_the_opponents_verified_mating_line() {
    let report = finish(job("8/8/8/8/8/5kq1/8/6RK w - - 0 1", "g1a1", config()));
    assert_eq!(report.plies[0].label, Label::AllowedMate);
    let row = &report.plies[0];
    let mut after = row.position_before.clone();
    after.play(row.played).unwrap();
    for &chess_move in &row.refutation.moves {
        after.play(chess_move).unwrap();
    }
    assert_eq!(after.status(), GameStatus::Checkmate { winner: Side::Onyx });
}
