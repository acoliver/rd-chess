use super::*;
use chess3d::search::{Limits, Score, Search};
use std::sync::atomic::AtomicBool;

#[test]
fn costly_quiet_move_names_the_alternative_and_preserves_both_lines() {
    let fen = "3r3k/8/8/8/8/8/8/3Q2NK w - - 0 1";
    let state = ChessState::from_fen(fen).unwrap();
    let mv = state.parse_uci("g1f3").unwrap();
    assert_large_score_gap(&state, mv);
    let result = explain(fen, "g1f3");
    assert_eq!(result.line[0], mv);
    let alternative = result.alternative_line.as_ref().unwrap();
    assert_eq!(state.to_san(alternative[0]).unwrap(), "Qxd8+");
    assert!(
        result.sentences.iter().any(|text| {
            text == "The search prefers Qxd8+: this line scores better in the current search."
        }),
        "{result:?}"
    );
    replay_all(fen, &result);
    let mut replay = state;
    for &mv in alternative {
        replay.play(mv).unwrap();
    }
}

#[test]
fn score_preference_does_not_require_a_missed_material_fact() {
    let fen = "7k/8/5n2/8/4P3/8/3P3P/6K1 w - - 0 1";
    let state = ChessState::from_fen(fen).unwrap();
    let mv = state.parse_uci("h2h3").unwrap();
    assert_large_score_gap(&state, mv);
    let result = explain(fen, "h2h3");
    assert!(
        !result.facts.iter().any(|fact| {
            matches!(
                fact.kind,
                FactKind::MissedMaterial { .. } | FactKind::MissedMate { .. }
            )
        }),
        "{result:?}"
    );
    assert_eq!(result.line[0], mv);
    assert_eq!(
        result.alternative_line.as_ref().unwrap()[0],
        state.parse_uci("e4e5").unwrap()
    );
    assert!(
        result.sentences.iter().any(|text| {
            text == "The search prefers e5: this line scores better in the current search."
        }),
        "{result:?}"
    );
    replay_all(fen, &result);
}

fn assert_large_score_gap(state: &ChessState, mv: chess3d::rules::LegalMove) {
    let analysis = Search::new(4096)
        .candidates(state.legal_moves().len())
        .analyze(
            state,
            Limits {
                nodes: (budget().nodes - 1) / 4,
                depth: Some(3),
            },
            &AtomicBool::new(false),
        );
    assert_eq!(analysis.completed_depth, 3);
    let played = analysis
        .candidates
        .iter()
        .find(|candidate| candidate.pv[0] == mv)
        .unwrap();
    let (Score::Centipawns(best), Score::Centipawns(played)) = (analysis.score, played.score)
    else {
        panic!("expected a centipawn comparison: {analysis:?}");
    };
    assert!(best - played > 100, "{best} versus {played}");
}

#[test]
fn missed_mate_still_names_an_alternative_without_a_centipawn_comparison() {
    let result = explain("7k/p7/5KQ1/8/8/8/8/8 w - - 0 1", "g6b1");
    assert!(result.alternative_line.is_some());
    assert!(
        result
            .sentences
            .iter()
            .any(|text| text.starts_with("The search prefers Qg7#:")),
        "{result:?}"
    );
}
