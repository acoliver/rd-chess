use super::*;

#[test]
fn checking_knight_fork_keeps_material_gain_when_capture_draws() {
    let fen = "r3k3/8/8/1N6/8/8/8/4K3 w - - 0 1";
    let result = explain(fen, "b5c7");
    assert_fact(
        &result,
        &FactKind::Fork {
            attacker: square("c7"),
            targets: vec![square("a8"), square("e8")],
        },
        "Nc7+: the knight forks the rook on a8 and the king on e8; the verified line wins the rook on a8.",
    );
    let fact = result
        .facts
        .iter()
        .find(|fact| matches!(fact.kind, FactKind::Fork { .. }))
        .unwrap();
    let mut state = ChessState::from_fen(fen).unwrap();
    for &mv in &fact.line {
        state.play(mv).unwrap();
    }
    assert_eq!(
        balance(&state, Side::Ivory) - balance(&ChessState::from_fen(fen).unwrap(), Side::Ivory),
        500
    );
    assert!(matches!(
        state.status(),
        chess3d::rules::GameStatus::Draw(_)
    ));
    replay_all(fen, &result);
}

#[test]
fn ordinary_forks_identify_the_targets_and_verify_the_gain() {
    for (fen, uci, attacker, targets) in [
        (
            "q3k3/8/8/1N6/8/8/8/4K3 w - - 0 1",
            "b5c7",
            "c7",
            ["a8", "e8"],
        ),
        (
            "7k/8/2n1n3/8/3P4/8/8/4K3 w - - 0 1",
            "d4d5",
            "d5",
            ["c6", "e6"],
        ),
        (
            "4k3/8/8/n7/8/8/8/3Q2K1 w - - 0 1",
            "d1a4",
            "a4",
            ["a5", "e8"],
        ),
    ] {
        let result = explain(fen, uci);
        let kind = FactKind::Fork {
            attacker: square(attacker),
            targets: targets.into_iter().map(square).collect(),
        };
        assert!(
            result.facts.iter().any(|fact| fact.kind == kind),
            "{fen}: {result:?}"
        );
        replay_all(fen, &result);
    }
}

#[test]
fn checking_fork_with_an_adequately_defended_target_claims_no_win() {
    let fen = "4k3/8/1p6/n7/8/8/8/3Q2K1 w - - 0 1";
    let result = explain(fen, "d1a4");
    assert!(
        !result
            .facts
            .iter()
            .any(|fact| matches!(fact.kind, FactKind::Fork { .. })),
        "{result:?}"
    );
    replay_all(fen, &result);
}
