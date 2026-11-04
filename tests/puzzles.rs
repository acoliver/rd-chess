use chess3d::rules::{ChessState, GameStatus};
use chess3d::teaching::{PuzzlePack, PuzzleRunner, TaskStatus};
use serde_json::json;

fn known() -> PuzzlePack {
    PuzzlePack::from_json(&json!({"version":1,"source":"https://database.lichess.org/lichess_db_puzzle.csv.zst",
        "puzzles":[{"PuzzleId":"00sHx","FEN":"q3k1nr/1pp1nQpp/3p4/1P2p3/4P3/B1PP1b2/B5PP/5K2 b k - 0 17",
        "Moves":"e8d7 a2e6 d7d8 f7f8","Rating":1760,"Themes":"mate mateIn2 middlegame short"}]}).to_string()).unwrap()
}

#[test]
fn known_csv_row_applies_opponents_setup_before_solver() {
    let pack = known();
    let mut runner = PuzzleRunner::new(pack.puzzles()[0].clone()).unwrap();
    assert_eq!(
        runner.board().to_fen(),
        "q5nr/1ppknQpp/3p4/1P2p3/4P3/B1PP1b2/B5PP/5K2 w - - 1 18"
    );
    runner.submit("a2e6").unwrap();
    assert_eq!(runner.status(), &TaskStatus::Correct);
    assert_eq!(runner.board().side_to_move(), chess3d::Side::Ivory);
    runner.submit("f7f8").unwrap();
    assert_eq!(runner.status(), &TaskStatus::Completed);
    assert_eq!(runner.progress().bands["1200-1799"].solved, 1);
    assert_eq!(runner.progress().bands["1200-1799"].attempts, 2);
    let progress = runner.progress();
    assert_eq!(
        chess3d::teaching::Progress::from_json(&progress.to_json().unwrap()).unwrap(),
        *progress
    );
}

#[test]
fn every_committed_puzzle_is_legal_and_mates_finish_in_mate() {
    let pack = PuzzlePack::bundled().unwrap();
    assert_eq!(pack.puzzles().len(), 300);
    for puzzle in pack.puzzles() {
        let mut state = ChessState::from_fen(&puzzle.fen).unwrap();
        for uci in puzzle.moves.split_whitespace() {
            state.play(state.parse_uci(uci).unwrap()).unwrap();
        }
        if puzzle.themes.split_whitespace().any(|t| t == "mate") {
            assert!(
                matches!(state.status(), GameStatus::Checkmate { .. }),
                "{}",
                puzzle.id
            );
        }
        let mut runner = PuzzleRunner::new(puzzle.clone()).unwrap();
        for uci in puzzle.moves.split_whitespace().skip(1).step_by(2) {
            runner.submit(uci).unwrap();
        }
        assert_eq!(runner.status(), &TaskStatus::Completed, "{}", puzzle.id);
    }
}

#[test]
fn puzzle_loader_rejects_bad_setup_and_non_mating_lines() {
    let pack = known();
    let mut value = serde_json::to_value(&pack).unwrap();
    value["puzzles"][0]["Moves"] = json!("e8e6 a2e6");
    assert!(PuzzlePack::from_json(&value.to_string()).is_err());
    value["puzzles"][0]["Moves"] = json!("e8d7 a2e6");
    assert!(PuzzlePack::from_json(&value.to_string()).is_err());
}

#[test]
fn alternate_mate_in_one_and_final_mate_in_two_are_accepted() {
    let pack = PuzzlePack::from_json(
        &json!({"version":1,"source":chess3d::teaching::PUZZLE_SOURCE,
        "puzzles":[{"PuzzleId":"original-test","FEN":"8/5K1k/8/6Q1/8/8/8/8 b - - 0 1",
        "Moves":"h7h8 g5g7","Rating":500,"Themes":"mate mateIn1"}]})
        .to_string(),
    )
    .unwrap();
    let puzzle = pack.puzzles()[0].clone();
    let mut runner = PuzzleRunner::new(puzzle).unwrap();
    runner.submit("g5h5").unwrap();
    assert_eq!(runner.status(), &TaskStatus::Completed);
    assert_eq!(runner.progress().bands["under-1200"].solved, 1);
    assert!(runner.submit("h8g8").is_err());
    assert_eq!(runner.progress().bands["under-1200"].solved, 1);
    assert_final_mate_alternatives();
}

fn assert_final_mate_alternatives() {
    let pack = PuzzlePack::bundled().unwrap();
    let mut alternatives = 0;
    for puzzle in pack
        .puzzles()
        .iter()
        .filter(|p| p.themes.split_whitespace().any(|t| t == "mateIn2"))
    {
        let moves: Vec<_> = puzzle.moves.split_whitespace().collect();
        let mut checkpoint = puzzle.after_setup().unwrap();
        for uci in &moves[1..moves.len() - 1] {
            checkpoint.play(checkpoint.parse_uci(uci).unwrap()).unwrap();
        }
        for candidate in checkpoint.legal_moves() {
            let mut after = checkpoint.clone();
            after.play(candidate).unwrap();
            if matches!(after.status(), GameStatus::Checkmate { .. })
                && candidate.to_uci() != moves[moves.len() - 1]
            {
                let mut runner = PuzzleRunner::new(puzzle.clone()).unwrap();
                runner.submit(moves[1]).unwrap();
                runner.submit(&candidate.to_uci()).unwrap();
                assert_eq!(runner.status(), &TaskStatus::Completed);
                alternatives += 1;
            }
        }
    }
    assert!(alternatives > 0, "pack must exercise alternate final mates");
}

#[test]
fn restored_puzzle_progress_retains_bands_and_wrong_moves_do_not_advance() {
    let pack = known();
    let puzzle = pack.puzzles()[0].clone();
    let mut runner = PuzzleRunner::new(puzzle.clone()).unwrap();
    let before = runner.board().to_fen();
    runner.submit("f1e1").unwrap();
    assert!(matches!(runner.status(), TaskStatus::Wrong(_)));
    assert_eq!(runner.board().to_fen(), before);
    let saved = runner.progress().to_json().unwrap();
    let mut restored = PuzzleRunner::with_progress(
        puzzle,
        chess3d::teaching::Progress::from_json(&saved).unwrap(),
    )
    .unwrap();
    restored.submit("a2e6").unwrap();
    restored.submit("f7f8").unwrap();
    assert_eq!(restored.progress().bands["1200-1799"].attempts, 3);
    assert_eq!(restored.progress().bands["1200-1799"].solved, 1);
    restored.retry().unwrap();
    assert_eq!(restored.board().to_fen(), before);
    assert_eq!(restored.status(), &TaskStatus::AwaitingMove);
}
