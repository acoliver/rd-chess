use super::*;
use crate::rules::{ChessState, INITIAL_FEN};
use std::sync::atomic::AtomicBool;
use std::time::Instant;

const POSITIONS: [&str; 10] = [
    INITIAL_FEN,
    "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
    "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1",
    "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 1 8",
    "4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 20",
    "4r1k1/8/8/8/1b6/8/8/R3K3 w - - 0 1",
    "1r2k3/P7/8/8/8/8/8/4K3 w - - 17 1",
    "4k3/8/8/8/8/8/3r4/3QK3 w - - 0 1",
    "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
];

fn record(case: usize, state: &ChessState, profile: &OpponentProfile) -> String {
    let analysis = Search::weighted(4096, profile.weights)
        .candidates(profile.candidates)
        .analyze(
            state,
            Limits {
                nodes: profile.nodes,
                depth: Some(profile.depth),
            },
            &AtomicBool::new(false),
        );
    format!(
        "{case} {} {analysis:?} reply={}\n",
        profile.name,
        choose_move(state, profile, 42).to_uci()
    )
}

// Recorded before optimization on 861d178 with the actual ladder budgets.
// Compare nodes, depth, scores, complete PVs/candidates and seed-42 replies.
#[test]
fn exact_ladder_baseline() {
    let mut expected = include_str!("fixtures/main-861d178.txt").lines();
    for (case, fen) in POSITIONS.iter().enumerate() {
        let state = ChessState::from_fen(fen).unwrap();
        for profile in LADDER {
            assert_eq!(
                record(case, &state, &profile).trim_end(),
                expected.next().unwrap(),
                "case {case} profile {}",
                profile.name
            );
        }
    }
    assert_eq!(expected.next(), None);
}

#[test]
#[ignore = "release fixed-position throughput measurement"]
fn fixed_throughput() {
    for run in 1..=3 {
        let start = Instant::now();
        let mut total = 0_u32;
        for (case, fen) in POSITIONS.iter().enumerate() {
            let state = ChessState::from_fen(fen).unwrap();
            let result = Search::new(65_536).analyze(
                &state,
                Limits {
                    nodes: 20_000,
                    depth: None,
                },
                &AtomicBool::new(false),
            );
            assert_eq!(result.nodes, 20_000, "case {case}");
            total += u32::try_from(result.nodes).unwrap();
            std::hint::black_box(result);
        }
        let elapsed = start.elapsed().as_secs_f64();
        println!(
            "search run={run} nodes={total} seconds={elapsed:.6} nps={:.0}",
            f64::from(total) / elapsed
        );
    }
}
