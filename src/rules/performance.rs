use super::*;
use std::hint::black_box;
use std::time::Instant;

fn perft(core: &Core, depth: u8) -> u32 {
    let moves = core.legal_moves();
    if depth == 1 {
        return u32::try_from(moves.len()).unwrap();
    }
    moves
        .into_iter()
        .map(|chess_move| perft(&core.after(chess_move), depth - 1))
        .sum()
}

#[test]
#[ignore = "release deep perft and three-run throughput measurement"]
fn deep_perft() {
    for (name, fen, depth, expected) in [
        ("initial", INITIAL_FEN, 5, 4_865_609),
        ("kiwipete", tests::KIWIPETE, 4, 4_085_603),
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        for run in 1..=3 {
            let start = Instant::now();
            let nodes = perft(black_box(&state.core), depth);
            let elapsed = start.elapsed().as_secs_f64();
            assert_eq!(nodes, expected);
            println!(
                "perft {name} run={run} nodes={nodes} seconds={elapsed:.6} nps={:.0}",
                f64::from(nodes) / elapsed
            );
        }
    }
}

#[test]
#[ignore = "release extended perft correctness checks"]
fn extended_perft() {
    for (fen, depth, expected) in [
        (INITIAL_FEN, 6, 119_060_324),
        (tests::KIWIPETE, 5, 193_690_690),
        (tests::POSITION_3, 6, 11_030_083),
        (tests::POSITION_4, 4, 422_333),
        (tests::POSITION_5, 4, 2_103_487),
    ] {
        let state = ChessState::from_fen(fen).unwrap();
        assert_eq!(perft(&state.core, depth), expected, "{fen} depth {depth}");
    }
}

fn measure(name: &str, mut action: impl FnMut()) {
    let start = Instant::now();
    for _ in 0..100_000 {
        action();
    }
    println!(
        "component {name} ns/call={:.1}",
        start.elapsed().as_secs_f64() * 10_000.0
    );
}

#[test]
#[ignore = "release isolated hot-path timings"]
fn components() {
    for (name, fen) in [("initial", INITIAL_FEN), ("kiwipete", tests::KIWIPETE)] {
        let mut state = ChessState::from_fen(fen).unwrap();
        let chess_move = state.legal_moves()[0];
        println!("position {name}");
        measure("legal_moves", || {
            black_box(black_box(&state).legal_moves());
        });
        measure("core_after", || {
            black_box(black_box(state.core).after(chess_move));
        });
        measure("hash", || {
            black_box(black_box(&state.core).zobrist());
        });
        measure("play_undo", || {
            state.play(black_box(chess_move)).unwrap();
            state.undo().unwrap();
            black_box(&state);
        });
        measure("generated_play_undo", || {
            state.play_generated(black_box(chess_move)).unwrap();
            black_box(&state);
            state.undo().unwrap();
            black_box(&state);
        });
        measure("evaluate", || {
            black_box(crate::search::evaluate(black_box(&state)));
        });
        measure("status", || {
            black_box(black_box(&state).status());
        });
    }
}
