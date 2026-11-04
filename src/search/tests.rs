#[test]
fn cancellable_policy_preserves_completed_iteration_and_stops_preprocessing() {
    use std::cell::Cell;
    let state = ChessState::initial();
    assert_eq!(choose_move_cancellable(&state, &PIP, 42, &|| true), None);
    let mut profile = PIP;
    profile.depth = 1;
    profile.nodes = 1_000_000;
    let polls = Cell::new(0);
    let first = choose_move_cancellable(&state, &profile, 42, &|| {
        polls.set(polls.get() + 1);
        false
    });
    assert!(first.is_some());
    let completed_polls = polls.get();
    polls.set(0);
    profile.depth = 2;
    let interrupted = choose_move_cancellable(&state, &profile, 42, &|| {
        polls.set(polls.get() + 1);
        polls.get() > completed_polls
    });
    assert_eq!(interrupted, first);
    assert!(polls.get() > completed_polls);
}

use super::*;
use crate::rules::{ChessState, GameStatus, INITIAL_FEN};
use crate::{PieceKind, Side};
use std::sync::atomic::{AtomicBool, Ordering};

const KIWIPETE: &str = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";

fn analyze(fen: &str, depth: u8) -> Analysis {
    Search::new(4096).analyze(
        &ChessState::from_fen(fen).unwrap(),
        Limits {
            nodes: 40_000,
            depth: Some(depth),
        },
        &AtomicBool::new(false),
    )
}

fn replay(state: &ChessState, line: &[crate::rules::LegalMove]) -> ChessState {
    let mut next = state.clone();
    for &chess_move in line {
        assert!(next.legal_moves().contains(&chess_move));
        next.play(chess_move).unwrap();
    }
    next
}

#[test]
fn finds_mates_with_legal_complete_lines() {
    // Original elementary KQK compositions, authored for this suite, not copied
    // from an engine. The king confines h8; the queen closes its escape squares.
    for (fen, plies) in [
        ("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", 1),
        ("7k/8/5K2/8/4Q3/8/8/8 w - - 0 1", 3),
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        let result = analyze(fen, 3);
        assert_eq!(
            result.score,
            Score::Mate {
                winner: Side::Ivory,
                plies
            }
        );
        assert_eq!(result.pv.len(), usize::from(plies));
        assert_eq!(
            replay(&state, &result.pv).status(),
            GameStatus::Checkmate {
                winner: Side::Ivory
            }
        );
    }
}

#[test]
fn stalemate_claims_and_history_score_zero() {
    for fen in [
        "7k/5Q2/6K1/8/8/8/8/8 b - - 0 1",
        "4k3/8/8/8/8/8/8/R3K3 w - - 100 80",
        "4k3/8/8/8/8/8/8/4K3 w - - 0 1",
    ] {
        assert_eq!(analyze(fen, 2).score, Score::Centipawns(0));
    }
    let mut state = ChessState::initial();
    for uci in ["g1f3", "g8f6", "f3g1", "f6g8"].repeat(2) {
        state.play(state.parse_uci(uci).unwrap()).unwrap();
    }
    let result = Search::new(128).analyze(
        &state,
        Limits {
            nodes: 200,
            depth: Some(2),
        },
        &AtomicBool::new(false),
    );
    assert_eq!(result.score, Score::Centipawns(0));
    assert!(result.pv.is_empty());
}

#[test]
fn captures_a_hanging_queen() {
    let result = analyze("4k3/8/8/8/8/8/3q4/3RK3 w - - 0 1", 2);
    assert_eq!(result.pv[0].to_uci(), "d1d2");
}

#[test]
fn budgets_are_exact_and_pvs_remain_legal() {
    for fen in [INITIAL_FEN, KIWIPETE, "4r1k1/8/8/8/1b6/8/8/R3K3 w - - 0 1"] {
        let state = ChessState::from_fen(fen).unwrap();
        for nodes in [0, 1, 64, 600] {
            let result = Search::new(128).analyze(
                &state,
                Limits { nodes, depth: None },
                &AtomicBool::new(false),
            );
            assert_eq!(result.nodes, nodes);
            assert!(!result.pv.is_empty());
            replay(&state, &result.pv);
        }
    }
}

#[test]
fn cancellation_preserves_last_completed_iteration() {
    let state = ChessState::initial();
    let cancel = AtomicBool::new(false);
    let expected = analyze(INITIAL_FEN, 1);
    let result = Search::new(4096).analyze_progress(
        &state,
        Limits {
            nodes: 30_000,
            depth: None,
        },
        &cancel,
        |_| cancel.store(true, Ordering::Relaxed),
    );
    assert_eq!(result.completed_depth, 1);
    assert_eq!(result.score, expected.score);
    assert_eq!(result.pv, expected.pv);
    assert!(result.nodes < 30_000);
    let stopped = Search::new(0).analyze(
        &state,
        Limits {
            nodes: 100,
            depth: None,
        },
        &cancel,
    );
    assert_eq!(stopped.nodes, 0);
    assert_eq!(stopped.completed_depth, 0);
    replay(&state, &stopped.pv);
}

#[test]
fn evaluation_is_colour_symmetric() {
    // Rank reflection plus colour exchange; neither position has castling rights.
    let white = ChessState::from_fen("4k3/pp3p2/2n5/3P4/2B5/8/PP3P2/3QK3 w - - 0 1").unwrap();
    let black = ChessState::from_fen("3qk3/pp3p2/8/2b5/3p4/2N5/PP3P2/4K3 b - - 0 1").unwrap();
    let left = evaluate(&white);
    let right = evaluate(&black);
    assert_eq!(left.material, -right.material);
    assert_eq!(left.placement, -right.placement);
    assert_eq!(left.mobility, -right.mobility);
    assert_eq!(left.king_safety, -right.king_safety);
    assert_eq!(left.pawns, -right.pawns);
    assert_eq!(
        left.total(Weights::BALANCED),
        -right.total(Weights::BALANCED)
    );
    assert_eq!(evaluate(&ChessState::initial()).total(Weights::BALANCED), 0);
}

#[test]
fn profiles_are_seeded_legal_and_handle_forced_evasions() {
    for fen in [
        INITIAL_FEN,
        "4r1k1/8/8/8/1b6/8/8/R3K3 w - - 0 1",
        "7k/8/5KQ1/8/8/8/8/8 w - - 0 1",
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        for profile in LADDER {
            // Exercise the same policy at a small reproducible test budget.
            let profile = OpponentProfile {
                nodes: 600,
                ..profile
            };
            let first = choose_move(&state, &profile, 42);
            assert_eq!(first, choose_move(&state, &profile, 42));
            for seed in [1, 2, 3] {
                assert!(
                    state
                        .legal_moves()
                        .contains(&choose_move(&state, &profile, seed))
                );
            }
        }
    }
}

#[test]
fn weak_policy_does_not_leave_a_queen_for_immediate_capture() {
    // Original fixtures: queen attacked by a rook, knight, or pawn respectively.
    for fen in [
        "4k3/8/8/8/8/8/3r4/3QK3 w - - 0 1",
        "4k3/8/8/8/8/2n5/8/3QK3 w - - 0 1",
        "4k3/8/8/8/8/8/2p5/3QK3 w - - 0 1",
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        for seed in 0..12 {
            let chess_move = choose_move(&state, &LADDER[0], seed);
            let next = replay(&state, &[chess_move]);
            for reply in next.legal_moves() {
                assert_ne!(
                    next.piece_at(reply.to()),
                    Some((Side::Ivory, PieceKind::Queen)),
                    "{fen}: {}",
                    chess_move.to_uci()
                );
            }
        }
    }
}

#[test]
#[ignore = "release throughput measurement, run with --release search::tests::throughput -- --ignored --nocapture"]
fn throughput() {
    for (name, fen) in [("initial", INITIAL_FEN), ("kiwipete", KIWIPETE)] {
        let state = ChessState::from_fen(fen).unwrap();
        let start = std::time::Instant::now();
        let result = Search::new(65_536).analyze(
            &state,
            Limits {
                nodes: 20_000,
                depth: None,
            },
            &AtomicBool::new(false),
        );
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "{name}: nodes={} elapsed={elapsed:.6}s nodes/second={:.1} depth={}",
            result.nodes,
            f64::from(u32::try_from(result.nodes).unwrap()) / elapsed,
            result.completed_depth
        );
        assert_eq!(result.nodes, 20_000);
        replay(&state, &result.pv);
    }
}

#[test]
fn a_forced_single_evasion_works_for_every_ladder_profile() {
    let state = ChessState::from_fen("r7/8/8/8/8/2k5/8/K7 w - - 0 1").unwrap();
    let moves = state.legal_moves();
    assert!(state.in_check());
    assert_eq!(moves.len(), 1);
    for profile in LADDER {
        let profile = OpponentProfile {
            nodes: 300,
            ..profile
        };
        assert_eq!(choose_move(&state, &profile, 123), moves[0]);
    }
}

#[test]
fn evaluation_features_are_individually_visible_and_weighted() {
    let state = ChessState::from_fen("4k3/pp3p2/2n5/3P4/2B5/8/PP6/3QK3 w - - 0 1").unwrap();
    let breakdown = evaluate(&state);
    for term in [
        breakdown.material,
        breakdown.placement,
        breakdown.mobility,
        breakdown.king_safety,
        breakdown.pawns,
    ] {
        assert_ne!(term, Term::default());
    }
    let only_material = Weights {
        material: 100,
        placement: 0,
        mobility: 0,
        king_safety: 0,
        pawns: 0,
    };
    let without_material = Weights {
        material: 0,
        ..Weights::BALANCED
    };
    assert_eq!(
        breakdown.total(Weights::BALANCED),
        breakdown.total(only_material) + breakdown.total(without_material)
    );
}

#[test]
fn top_n_bounds_match_full_window_candidate_scores() {
    let state = ChessState::initial();
    let limits = Limits {
        nodes: 15_000,
        depth: Some(2),
    };
    let cancel = AtomicBool::new(false);
    let reference = Search::new(4096)
        .candidates(usize::MAX)
        .analyze(&state, limits, &cancel);
    assert_eq!(reference.completed_depth, 2);
    for width in [1, 2, 4] {
        let result = Search::new(4096)
            .candidates(width)
            .analyze(&state, limits, &cancel);
        assert_eq!(result.completed_depth, 2);
        let scores: Vec<_> = result
            .candidates
            .iter()
            .map(|candidate| candidate.score)
            .collect();
        let expected: Vec<_> = reference
            .candidates
            .iter()
            .take(width)
            .map(|candidate| candidate.score)
            .collect();
        assert_eq!(scores, expected);
        for candidate in &result.candidates {
            replay(&state, &candidate.pv);
        }
    }
}

#[test]
fn mates_keep_absolute_winners_for_black_and_the_losing_side() {
    // Colour/rank reflection of the original elementary KQK composition.
    let black = analyze("8/8/8/8/8/5kq1/8/7K b - - 0 1", 2);
    assert_eq!(
        black.score,
        Score::Mate {
            winner: Side::Onyx,
            plies: 1
        }
    );
    let lost = analyze("7k/6Q1/6K1/8/8/8/8/8 b - - 100 80", 2);
    assert_eq!(
        lost.score,
        Score::Mate {
            winner: Side::Ivory,
            plies: 0
        }
    );
    assert!(lost.pv.is_empty());
}

#[test]
fn transpositions_do_not_change_exact_root_scores_or_lines() {
    for fen in [
        "7k/8/5K2/8/4Q3/8/8/8 w - - 0 1",
        "4k3/8/8/8/8/8/3q4/3RK3 w - - 0 1",
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        let expected = analyze(fen, 2);
        for capacity in [0, 1, 128] {
            let mut search = Search::new(capacity);
            for _ in 0..2 {
                let result = search.analyze(
                    &state,
                    Limits {
                        nodes: 40_000,
                        depth: Some(2),
                    },
                    &AtomicBool::new(false),
                );
                assert_eq!(result.completed_depth, 2);
                assert_eq!(result.score, expected.score);
                replay(&state, &result.pv);
                assert_eq!(result.candidates.len(), 1);
            }
        }
    }
}

#[test]
fn an_interrupted_iteration_does_not_publish_partial_candidates() {
    let state = ChessState::initial();
    let expected = analyze(INITIAL_FEN, 1);
    let budget = expected.nodes + 5;
    let result = Search::new(4096).analyze(
        &state,
        Limits {
            nodes: budget,
            depth: None,
        },
        &AtomicBool::new(false),
    );
    assert_eq!(result.nodes, budget);
    assert_eq!(result.completed_depth, 1);
    assert_eq!(result.pv, expected.pv);
    assert_eq!(result.candidates, expected.candidates);
    assert_eq!(state, ChessState::initial());
}

#[test]
fn seeded_variation_stays_in_the_profile_candidate_set() {
    let state = ChessState::initial();
    let profile = OpponentProfile {
        nodes: 600,
        ..LADDER[0]
    };
    let result = Search::weighted(4096, profile.weights)
        .candidates(profile.candidates)
        .analyze(
            &state,
            Limits {
                nodes: profile.nodes,
                depth: Some(profile.depth),
            },
            &AtomicBool::new(false),
        );
    assert!(result.completed_depth > 0);
    let top: Vec<_> = result
        .candidates
        .iter()
        .take(profile.candidates)
        .map(|candidate| candidate.pv[0])
        .collect();
    let chosen: std::collections::HashSet<_> = (0..8)
        .map(|seed| {
            let chess_move = choose_move(&state, &profile, seed);
            assert!(top.contains(&chess_move));
            chess_move
        })
        .collect();
    assert!(chosen.len() > 1);
}
