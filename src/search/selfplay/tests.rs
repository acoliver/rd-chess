#[test]
fn first_games_profile_allows_an_elementary_recapture() {
    let mut state = opening(OPENINGS.lines().next().unwrap());
    let decision = crate::search::choose_move_measured(&state, &PIP, mix(mix(1925)));
    assert_eq!(decision.chess_move.to_uci(), "c4f7");
    state.play(decision.chess_move).unwrap();
    state.play(state.parse_uci("e8f7").unwrap()).unwrap();
    assert!(crate::search::evaluate(&state).material.middlegame < -200);
}

#[test]
fn measured_policy_is_the_production_choice_and_counts_only_search_nodes() {
    let state = crate::rules::ChessState::initial();
    let decision = crate::search::choose_move_measured(&state, &PIP, 42);
    assert_eq!(
        decision.chess_move,
        crate::search::choose_move(&state, &PIP, 42)
    );
    assert!(decision.nodes <= PIP.nodes);
    assert!(decision.nodes > 0);
}

#[test]
fn mate_precedes_claims_and_all_draws_stop_play() {
    let mate = ChessState::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 150 80").unwrap();
    assert_eq!(outcome(&mate), Some(Finish::Mate(Side::Ivory)));
    let dead = ChessState::from_fen("4k3/8/8/8/8/8/8/4K3 w - - 0 1").unwrap();
    assert_eq!(outcome(&dead), Some(Finish::RulesDraw));
    let claim = ChessState::from_fen("4k3/8/8/8/8/8/8/R3K3 w - - 100 80").unwrap();
    assert_eq!(outcome(&claim), Some(Finish::ClaimDraw));
}

use super::*;
use crate::search::{LADDER, PIP};

#[test]
fn fixed_short_match_repeats_moves_results_and_nodes_across_workers() {
    let profiles = [PIP, LADDER[1]];
    let config = MatchConfig {
        games: 4,
        threads: 1,
        max_plies: 8,
        seed: 42,
    };
    let first = play_match(profiles, config).unwrap();
    let second = play_match(
        profiles,
        MatchConfig {
            threads: 2,
            ..config
        },
    )
    .unwrap();
    for (left, right) in first.iter().zip(&second) {
        assert_eq!(left.index, right.index);
        assert_eq!(left.moves, right.moves);
        assert_eq!(left.finish, right.finish);
        assert_eq!(left.stats[0].nodes, right.stats[0].nodes);
        assert_eq!(left.stats[1].nodes, right.stats[1].nodes);
    }
    assert_eq!(first.len(), 4);
    let position = opening(OPENINGS.lines().next().unwrap());
    let seed = mix(mix(config.seed));
    for player in 0..2 {
        assert_eq!(
            first[player].moves[0],
            crate::search::choose_move(&position, &profiles[player], seed)
        );
    }
}

#[test]
fn openings_are_legal_quiet_and_material_balanced() {
    assert_eq!(OPENINGS.lines().count(), 20);
    for line in OPENINGS.lines() {
        let state = opening(line);
        assert!(!state.in_check(), "{line}");
        assert_eq!(super::super::evaluate(&state).material.middlegame, 0);
        assert!(
            super::super::evaluate(&state)
                .total(super::super::Weights::BALANCED)
                .abs()
                < 100
        );
    }
}

#[test]
fn rejects_unpaired_games_and_unbounded_workers() {
    for (games, threads, max_plies) in [(0, 1, 8), (3, 1, 8), (2, 0, 8), (2, 7, 8), (2, 1, 0)] {
        assert!(
            play_match(
                [PIP, PIP],
                MatchConfig {
                    games,
                    threads,
                    max_plies,
                    seed: 0
                }
            )
            .is_err()
        );
    }
}

#[test]
fn scoring_and_interval_use_colour_pairs() {
    let game = |index, finish| Game {
        index,
        moves: Vec::new(),
        finish,
        stats: [MoveStats::default(); 2],
    };
    let games = [game(0, Finish::Mate(Side::Onyx)), game(1, Finish::MoveCap)];
    let report = MatchReport::new(&games);
    assert_eq!((report.wins, report.draws, report.losses), (1, 1, 0));
    assert!((report.score - 0.75).abs() < f64::EPSILON);
    assert!(report.interval[0] < 0.75 && report.interval[1] >= 0.75);
}

#[test]
#[ignore = "release self-play smoke: cargo test --release -p chess3d --lib search::selfplay::tests::strongest -- --ignored --nocapture"]
fn strongest_wins_majority_against_first_games_profile() {
    let games = play_match(
        [LADDER[0], LADDER[7]],
        MatchConfig {
            games: 6,
            threads: 3,
            max_plies: 200,
            seed: 2026,
        },
    )
    .unwrap();
    let report = MatchReport::new(&games);
    println!(
        "wins={} draws={} losses={}",
        report.wins, report.draws, report.losses
    );
    assert!(report.wins > 3);
}
