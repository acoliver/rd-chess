use super::*;
use chess3d::teaching::Explanation;

#[test]
fn all_teaching_fixture_sentences_exclude_engine_diagnostics_and_centipawns() {
    let direct = direct_cases()
        .into_iter()
        .map(|(fen, uci, _, _)| (fen, uci));
    let tactical = tactical_cases()
        .into_iter()
        .map(|(fen, uci, _, _)| (fen, uci));
    for (fen, uci) in direct.chain(tactical) {
        assert_learner_sentences(&explain(fen, uci));
    }
}

#[test]
fn checking_fork_subsumes_its_threat_and_direct_check() {
    let fen = "r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1";
    let result = explain(fen, "b5c7");
    assert_eq!(result.facts.len(), 1, "{result:?}");
    assert!(matches!(result.facts[0].kind, FactKind::Fork { .. }));
    assert_eq!(
        result.sentences,
        ["Nc7+ forks the king on e8 and the rook on a8 and wins a rook in this line."]
    );
    assert_eq!(result.facts[0].line, result.line);
    replay_all(fen, &result);
}

#[test]
fn clean_captures_name_pieces_but_exchanges_use_net_pawns() {
    for (fen, uci, expected) in [
        ("r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5c7", "wins a rook"),
        ("q3k3/8/8/1N6/8/8/8/4K3 w - - 0 1", "b5c7", "wins a queen"),
        ("4k3/8/8/n7/8/8/8/3Q2K1 w - - 0 1", "d1a4", "wins a knight"),
        (
            "7k/8/2n1n3/8/3P4/8/8/4K3 w - - 0 1",
            "d4d5",
            "wins about 2.2 pawns",
        ),
        (
            "3r3k/8/8/3p4/4Q3/8/8/6K1 w - - 0 1",
            "e4d5",
            "loses about 8.5 pawns",
        ),
    ] {
        let result = explain(fen, uci);
        assert!(
            result.sentences.iter().any(|text| text.contains(expected)),
            "{result:?}"
        );
        replay_all(fen, &result);
    }
}

#[test]
fn caveats_are_short_and_only_report_a_relevant_limit() {
    let fork = explain("4k3/4n3/8/8/1B6/8/8/R5K1 w - - 0 1", "a1e1");
    assert_eq!(fork.sentences.last().unwrap(), "Checked 3 moves ahead.");
    let direct = explain("7k/8/5KQ1/8/8/8/8/8 w - - 0 1", "g6g7");
    assert_eq!(direct.sentences, ["Qg7# is checkmate."]);
    let state = ChessState::initial();
    let unresolved = explain_move(
        &state,
        state.parse_uci("g1f3").unwrap(),
        Budget { nodes: 0, depth: 3 },
    )
    .unwrap();
    assert_eq!(
        unresolved.sentences,
        ["The search could not settle this line."]
    );
    assert_learner_sentences(&fork);
    assert_learner_sentences(&direct);
    assert_learner_sentences(&unresolved);
}

pub(super) fn assert_learner_sentences(result: &Explanation) {
    for sentence in &result.sentences {
        let text = sentence.to_lowercase();
        for forbidden in [
            "centipawn",
            "analysis limits:",
            "exchange facts cover",
            "recursion cap",
            "blunder",
        ] {
            assert!(!text.contains(forbidden), "{sentence}");
        }
    }
    let caveats = result
        .sentences
        .iter()
        .filter(|sentence| {
            sentence.starts_with("Checked ")
                || sentence.as_str() == "The search could not settle this line."
        })
        .count();
    assert!(caveats <= 1, "{result:?}");
}
