#[test]
fn unresolved_forks_do_not_get_a_tactical_claim() {
    let state = ChessState::from_fen(tactical_cases()[0].0).unwrap();
    let mv = state.parse_uci("c5e6").unwrap();
    for nodes in [0, 1, 5] {
        let result = explain_move(&state, mv, Budget { nodes, depth: 3 }).unwrap();
        assert!(result.limits.nodes <= nodes);
        assert!(result.limits.unresolved);
        assert!(
            !result
                .facts
                .iter()
                .any(|fact| matches!(fact.kind, FactKind::Fork { .. }))
        );
        assert!(
            result
                .sentences
                .iter()
                .any(|text| text == "The tactic is unresolved within the analysis budget.")
        );
        assert!(
            result
                .sentences
                .iter()
                .all(|text| !text.contains("blunder"))
        );
    }
    let resolved = explain_move(&state, mv, budget()).unwrap();
    assert_eq!(resolved, explain_reply(&state, mv, budget()).unwrap());
    assert!(
        resolved
            .facts
            .iter()
            .any(|fact| matches!(fact.kind, FactKind::Fork { .. }))
    );
}

#[test]
fn equal_best_scores_do_not_claim_that_another_line_scores_better() {
    let state = ChessState::initial();
    let result = explain_move(&state, state.parse_uci("e2e3").unwrap(), budget()).unwrap();
    assert!(result.facts.is_empty(), "{result:?}");
    assert_eq!(result.sentences, ["No verified tactical motif was found."]);
}

#[test]
fn discovered_attack_material_threat_and_missed_material_have_exact_facts() {
    let discovered = explain("4k3/4q3/5P2/8/8/8/4B3/4R1K1 w - - 0 1", "e2h5");
    assert_fact(
        &discovered,
        &FactKind::DiscoveredAttack {
            attacker: square("e1"),
            target: square("e7"),
            piece: PieceKind::Rook,
        },
        "Bh5+ uncovers the rook on e1 against e7; the verified line wins material.",
    );
    let fork = explain(tactical_cases()[0].0, "c5e6");
    assert_fact(
        &fork,
        &FactKind::Fork {
            attacker: square("e6"),
            targets: vec![square("c7"), square("g7")],
        },
        "Ne6+ forks c7 and g7; the verified line wins material.",
    );
    assert_fact(
        &fork,
        &FactKind::MaterialThreat { gain: 950 },
        "Ne6+ threatens material next move; the verified line wins 950 centipawns.",
    );
    let fen = "3r3k/8/8/8/8/8/8/3Q2K1 w - - 0 1";
    let missed = explain(fen, "d1d4");
    let state = ChessState::from_fen(fen).unwrap();
    assert_fact(
        &missed,
        &FactKind::MissedMaterial {
            alternative: state.parse_uci("d1d8").unwrap(),
            gain: 500,
        },
        "Qd4+ misses Qxd8+; its verified line wins 500 centipawns.",
    );
}

fn assert_fact(result: &chess3d::teaching::Explanation, kind: &FactKind, text: &str) {
    let index = result
        .facts
        .iter()
        .position(|fact| &fact.kind == kind)
        .unwrap_or_else(|| panic!("missing {kind:?}: {result:?}"));
    assert_eq!(result.sentences[index], text);
}

#[test]
fn pinned_defenders_and_defended_targets_use_legal_exchange_sequences() {
    let result = explain("8/6k1/5n2/3pB3/4P3/8/8/4K3 w - - 0 1", "e4d5");
    assert_fact(
        &result,
        &FactKind::Exchange { gain: 100 },
        "exd5 wins 100 centipawns in the verified exchange.",
    );
    let defended = explain("3r4/7k/8/8/8/2B5/8/3R2K1 w - - 0 1", "d1d4");
    assert!(!defended.facts.iter().any(
        |fact| matches!(fact.kind, FactKind::HangingPiece { square: sq, .. } if sq == square("d4"))
    ));
}

#[test]
fn en_passant_and_capture_promotion_record_actual_victims_and_balance() {
    let ep = explain("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2", "e5d6");
    assert_fact(
        &ep,
        &FactKind::Capture {
            attacker: PieceKind::Pawn,
            victim: PieceKind::Pawn,
            square: square("d5"),
            balance_change: 100,
        },
        "exd6 captures the pawn on d5 with a pawn; material balance changes by +100 centipawns.",
    );
    let promoted = explain("1r2k3/P7/8/8/8/8/8/4K3 w - - 0 1", "a7b8q");
    assert_fact(
        &promoted,
        &FactKind::Capture {
            attacker: PieceKind::Pawn,
            victim: PieceKind::Rook,
            square: square("b8"),
            balance_change: 1350,
        },
        "axb8=Q+ captures the rook on b8 with a pawn; material balance changes by +1350 centipawns.",
    );
}

#[test]
fn no_evidence_is_claimed_without_nodes_and_inputs_are_unchanged() {
    let state = ChessState::from_fen("4k3/8/8/8/8/8/4B3/4R1K1 w - - 0 1").unwrap();
    let saved = state.clone();
    let mv = state.parse_uci("e2f3").unwrap();
    let result = explain_move(&state, mv, Budget { nodes: 0, depth: 3 }).unwrap();
    assert!(result.facts.is_empty());
    assert_eq!(result.limits.nodes, 0);
    assert_eq!(state, saved);
    let foreign_move = ChessState::initial().parse_uci("e2e4").unwrap();
    assert!(explain_move(&state, foreign_move, budget()).is_err());
}

use chess3d::rules::ChessState;
use chess3d::teaching::{Budget, FactKind, explain_move, explain_reply};
use chess3d::{PieceKind, Side, Square};

fn square(name: &str) -> Square {
    Square::new(name.as_bytes()[0] - b'a', name.as_bytes()[1] - b'1').unwrap()
}

fn explain(fen: &str, uci: &str) -> chess3d::teaching::Explanation {
    let state = ChessState::from_fen(fen).unwrap();
    explain_move(
        &state,
        state
            .parse_uci(uci)
            .unwrap_or_else(|error| panic!("{fen}: {uci}: {error}")),
        budget(),
    )
    .unwrap()
}

fn budget() -> Budget {
    Budget {
        nodes: 100_000,
        depth: 3,
    }
}

#[test]
fn direct_facts_have_exact_templates_and_replayable_evidence() {
    let cases = direct_cases();
    for (fen, uci, kind, sentence) in cases {
        let result = explain(fen, uci);
        let index = result
            .facts
            .iter()
            .position(|fact| fact.kind == kind)
            .unwrap();
        assert_eq!(result.sentences[index], sentence);
        replay_all(fen, &result);
    }
}

fn direct_cases() -> [(&'static str, &'static str, FactKind, &'static str); 6] {
    [
        (
            "4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1",
            "e4d5",
            FactKind::Capture {
                attacker: PieceKind::Pawn,
                victim: PieceKind::Pawn,
                square: square("d5"),
                balance_change: 100,
            },
            "exd5 captures the pawn on d5 with a pawn; material balance changes by +100 centipawns.",
        ),
        (
            "4k3/8/8/8/8/8/8/R5K1 w - - 0 1",
            "a1e1",
            FactKind::Check,
            "Re1+ gives check.",
        ),
        (
            "7k/8/5KQ1/8/8/8/8/8 w - - 0 1",
            "g6g7",
            FactKind::Checkmate {
                winner: Side::Ivory,
            },
            "Qg7# is checkmate.",
        ),
        (
            "7k/P7/8/8/8/8/8/4K3 w - - 0 1",
            "a7a8q",
            FactKind::Promotion {
                into: PieceKind::Queen,
            },
            "a8=Q+ promotes the pawn to a queen.",
        ),
        (
            "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1",
            "e1g1",
            FactKind::Castling { king_side: true },
            "O-O castles kingside.",
        ),
        (
            "4k3/8/8/8/8/8/8/R3K2R w KQ - 0 1",
            "e1f1",
            FactKind::CastlingRightsLost {
                side: Side::Ivory,
                king_side: true,
                queen_side: true,
            },
            "Kf1 loses White's kingside and queenside castling rights.",
        ),
    ]
}

#[test]
fn original_tactical_positions_have_verified_motif_templates() {
    for (fen, uci, label, sentence) in tactical_cases() {
        let result = explain(fen, uci);
        let kind = tactical_kind(label, sentence);
        assert_eq!(kind.label(), label);
        assert_fact(&result, &kind, sentence);
        replay_all(fen, &result);
    }
}

fn tactical_kind(label: &str, sentence: &str) -> FactKind {
    match label {
        "fork" => FactKind::Fork {
            attacker: square("e6"),
            targets: vec![square("c7"), square("g7")],
        },
        "absolute pin" | "relative pin" => FactKind::Pin {
            attacker: square("e1"),
            front: square("e7"),
            rear: square("e8"),
            rear_piece: if label == "absolute pin" {
                PieceKind::King
            } else {
                PieceKind::Rook
            },
        },
        "skewer" => FactKind::Skewer {
            attacker: square("e1"),
            front: square("e7"),
            rear: square("e8"),
        },
        "discovered check" => FactKind::DiscoveredCheck {
            attacker: square("e1"),
            piece: PieceKind::Rook,
        },
        "hanging piece" => FactKind::HangingPiece {
            piece: PieceKind::Queen,
            square: square("d4"),
            winner: Side::Onyx,
            gain: 950,
        },
        "exchange" => FactKind::Exchange {
            gain: if sentence.contains("loses") {
                -850
            } else {
                100
            },
        },
        "mate threat" => FactKind::MateThreat,
        _ => panic!("unknown fixture label {label}"),
    }
}

fn tactical_cases() -> [(&'static str, &'static str, &'static str, &'static str); 9] {
    [
        (
            "8/2k3q1/8/2N5/8/8/P7/4K3 w - - 0 1",
            "c5e6",
            "fork",
            "Ne6+ forks c7 and g7; the verified line wins material.",
        ),
        (
            "4k3/4n3/8/8/1B6/8/8/R5K1 w - - 0 1",
            "a1e1",
            "absolute pin",
            "Re1 pins e7 to the king on e8; the verified line wins material.",
        ),
        (
            "4r2k/4n3/3P4/8/8/8/5PPP/R5K1 w - - 0 1",
            "a1e1",
            "relative pin",
            "Re1 pins e7 to the rook on e8; the verified line wins material.",
        ),
        (
            "4r3/4k3/8/1B6/8/8/8/R5K1 w - - 0 1",
            "a1e1",
            "skewer",
            "Re1+ skewers e7 and e8; the verified line wins material.",
        ),
        (
            "4k3/8/8/8/8/8/4B3/4R1K1 w - - 0 1",
            "e2f3",
            "discovered check",
            "Bf3+ uncovers check from the rook on e1.",
        ),
        (
            "3r3k/8/8/8/8/8/8/3Q2K1 w - - 0 1",
            "d1d4",
            "hanging piece",
            "Qd4+ leaves the queen on d4 hanging: the verified exchange wins 950 centipawns for Black.",
        ),
        (
            "4k3/8/8/3p4/4P3/8/8/4K3 w - - 0 1",
            "e4d5",
            "exchange",
            "exd5 wins 100 centipawns in the verified exchange.",
        ),
        (
            "3r3k/8/8/3p4/4Q3/8/8/6K1 w - - 0 1",
            "e4d5",
            "exchange",
            "Qxd5 loses 850 centipawns in the verified exchange.",
        ),
        (
            "7k/p7/5K2/8/8/3Q4/8/8 w - - 0 1",
            "d3g6",
            "mate threat",
            "Qg6 creates a mate-in-one threat; the verified line ends in checkmate.",
        ),
    ]
}

#[test]
fn missed_mate_names_the_search_alternative_and_its_line() {
    let fen = "7k/p7/5KQ1/8/8/8/8/8 w - - 0 1";
    let result = explain(fen, "g6b1");
    assert!(
        result
            .facts
            .iter()
            .any(|fact| matches!(fact.kind, FactKind::MissedMate { .. }))
    );
    assert!(
        result
            .sentences
            .iter()
            .any(|text| text == "Qb1 misses Qg7#, which gives checkmate in the verified line.")
    );
    replay_all(fen, &result);
}

#[test]
fn superficial_forks_and_unexploitable_pins_are_not_claimed() {
    for (fen, uci) in [
        ("4k3/2r1q1r1/8/2N5/8/8/8/K7 w - - 0 1", "c5e6"),
        ("4k3/4n3/8/8/8/8/8/R5K1 w - - 0 1", "a1e1"),
    ] {
        let result = explain(fen, uci);
        assert!(
            !result
                .facts
                .iter()
                .any(|fact| matches!(fact.kind, FactKind::Fork { .. } | FactKind::Pin { .. })),
            "{result:?}"
        );
    }
}

#[test]
fn quiet_positions_use_neutral_wording_without_a_fabricated_cause() {
    let state = ChessState::initial();
    let result = explain_move(&state, state.parse_uci("a2a3").unwrap(), budget()).unwrap();
    assert!(result.facts.is_empty(), "{result:?}");
    assert_eq!(
        result.sentences[0],
        "This line scores better in the current search"
    );
    assert!(!result.line.is_empty());
    assert!(result.limits.nodes <= budget().nodes);
}

#[test]
fn deterministic_shared_budget_and_reply_api() {
    let state = ChessState::initial();
    let mv = state.parse_uci("g1f3").unwrap();
    for nodes in [0, 1, 2, 10, 100] {
        let limits = Budget { nodes, depth: 3 };
        let first = explain_move(&state, mv, limits).unwrap();
        assert_eq!(first, explain_move(&state, mv, limits).unwrap());
        assert_eq!(first, explain_reply(&state, mv, limits).unwrap());
        assert!(first.limits.nodes <= nodes);
        assert!(first.limits.unresolved);
        assert!(
            first
                .sentences
                .iter()
                .any(|text| text == "The tactic is unresolved within the analysis budget.")
        );
    }
}

fn replay_all(fen: &str, result: &chess3d::teaching::Explanation) {
    for fact in &result.facts {
        let mut state = ChessState::from_fen(fen).unwrap();
        assert!(!fact.line.is_empty());
        for &mv in &fact.line {
            state.play(mv).unwrap();
        }
        let original = ChessState::from_fen(fen).unwrap();
        verify_premise(&original, &state, fact);
        assert!(fact.line.len() <= usize::from(result.limits.budget.depth.min(6)) + 8);
    }
}

fn verify_premise(before: &ChessState, after: &ChessState, fact: &chess3d::teaching::Fact) {
    let gain = balance(after, before.side_to_move()) - balance(before, before.side_to_move());
    match &fact.kind {
        FactKind::Exchange { gain: expected }
        | FactKind::MissedMaterial { gain: expected, .. }
        | FactKind::MaterialThreat { gain: expected } => assert_eq!(gain, *expected),
        FactKind::HangingPiece {
            gain: expected,
            winner,
            ..
        } => {
            let mut root = before.clone();
            root.play(fact.line[0]).unwrap();
            assert_eq!(balance(after, *winner) - balance(&root, *winner), *expected);
        }
        FactKind::Fork { .. }
        | FactKind::Pin { .. }
        | FactKind::Skewer { .. }
        | FactKind::DiscoveredAttack { .. } => assert!(gain > 0),
        FactKind::Checkmate { .. } | FactKind::MissedMate { .. } | FactKind::MateThreat => {
            assert!(
                matches!(after.status(), chess3d::rules::GameStatus::Checkmate { winner } if winner == before.side_to_move())
            );
        }
        FactKind::Check | FactKind::DiscoveredCheck { .. } => assert!(after.in_check()),
        _ => (),
    }
}

fn balance(state: &ChessState, side: Side) -> i32 {
    (0..8)
        .flat_map(|rank| (0..8).map(move |file| Square { rank, file }))
        .filter_map(|sq| state.piece_at(sq))
        .map(|(owner, kind)| {
            let value = match kind {
                PieceKind::Pawn => 100,
                PieceKind::Knight => 320,
                PieceKind::Bishop => 335,
                PieceKind::Rook => 500,
                PieceKind::Queen => 950,
                PieceKind::King => 0,
            };
            if owner == side { value } else { -value }
        })
        .sum()
}
