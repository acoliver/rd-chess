use super::board::{attacks, victim};
use super::verify::{Meter, Proof, prove};
use super::{
    AnalysisLimits, Budget, Explanation, Fact, FactKind, add, direct, exchange, motifs, templates,
};
use crate::rules::{ChessState, GameStatus, LegalMove};
use crate::search::{Analysis, Limits, Search};
use std::sync::atomic::AtomicBool;

pub(super) fn explain(
    state: &ChessState,
    after: &ChessState,
    mv: LegalMove,
    budget: Budget,
) -> Explanation {
    let mut meter = Meter {
        budget,
        nodes: 0,
        unresolved: false,
    };
    let admitted = meter.visit().is_some();
    let mut facts = if admitted {
        direct::facts(state, after, mv)
    } else {
        Vec::new()
    };
    let analysis = alternative(state, &mut meter);
    exchange::facts(state, after, mv, &mut meter, &mut facts);
    let played = prove(state, mv, &mut meter);
    if admitted {
        motifs::facts(state, after, mv, played.as_ref(), &mut facts);
    }
    if let Some(proof) = &played {
        threat(state, mv, proof, &mut meter, &mut facts);
    }
    let best = analysis.pv.first().copied().filter(|&best| best != mv);
    if let Some(best) = best.filter(|_| analysis.completed_depth > 0)
        && let Some(proof) = prove(state, best, &mut meter)
    {
        missed(best, &proof, played.as_ref(), &mut facts);
    }
    let better = scores_better(&analysis, mv);
    let line = if facts.is_empty() && better {
        analysis.pv.clone()
    } else {
        played
            .as_ref()
            .map_or_else(|| vec![mv], |proof| proof.line.clone())
    };
    finish(
        state,
        mv,
        facts,
        line,
        AnalysisLimits {
            budget,
            nodes: meter.nodes,
            verified_depth: if played.is_some() {
                budget.depth.min(6)
            } else {
                0
            },
            alternative_depth: analysis.completed_depth,
            unresolved: meter.unresolved,
        },
        better,
    )
}

fn finish(
    state: &ChessState,
    mv: LegalMove,
    facts: Vec<Fact>,
    line: Vec<LegalMove>,
    limits: AnalysisLimits,
    better: bool,
) -> Explanation {
    let mut sentences: Vec<_> = facts
        .iter()
        .map(|fact| templates::sentence(state, mv, fact))
        .collect();
    if facts.is_empty() && better {
        sentences.push("This line scores better in the current search".into());
    }
    if sentences.is_empty() && !limits.unresolved {
        sentences.push("No verified tactical motif was found.".into());
    }
    if limits.unresolved {
        sentences.push("The tactic is unresolved within the analysis budget.".into());
    }
    Explanation {
        facts,
        sentences,
        line,
        limits,
    }
}

fn alternative(state: &ChessState, meter: &mut Meter) -> Analysis {
    let limits = Limits {
        nodes: (meter.budget.nodes - meter.nodes) / 4,
        depth: Some(meter.budget.depth.min(6)),
    };
    let analysis = Search::new(4096)
        .candidates(state.legal_moves().len())
        .analyze(state, limits, &AtomicBool::new(false));
    meter.nodes += analysis.nodes;
    if analysis.completed_depth < meter.budget.depth.min(6) {
        meter.unresolved = true;
    }
    analysis
}

fn missed(best: LegalMove, proof: &Proof, played: Option<&Proof>, facts: &mut Vec<Fact>) {
    let Some(played) = played else {
        return;
    };
    if proof.wins_mate() && !played.wins_mate() {
        add(
            facts,
            FactKind::MissedMate { alternative: best },
            &proof.line,
        );
    } else if proof.gain() > 0 && played.gain() <= 0 && !played.wins_mate() {
        add(
            facts,
            FactKind::MissedMaterial {
                alternative: best,
                gain: proof.gain(),
            },
            &proof.line,
        );
    }
}

fn threat(
    before: &ChessState,
    mv: LegalMove,
    proof: &Proof,
    meter: &mut Meter,
    facts: &mut Vec<Fact>,
) {
    if victim(before, mv).is_some() || mv.promotion().is_some() || proof.line.len() < 3 {
        return;
    }
    let mut position = before.clone();
    for &step in &proof.line[..2] {
        position.play(step).expect("verified line");
    }
    let follow = proof.line[2];
    let source = if follow.from() == mv.to() {
        mv.from()
    } else {
        follow.from()
    };
    let mut final_position = position.clone();
    final_position.play(follow).expect("verified follow-up");
    let mate = matches!(final_position.status(), GameStatus::Checkmate { winner } if winner == before.side_to_move());
    if mate && proof.wins_mate() && existing_mate(before, source, follow, meter) == Some(false) {
        add(facts, FactKind::MateThreat, &proof.line);
    } else if proof.gain() > 0
        && victim(&position, follow).is_some()
        && !attacks(before, source, follow.to())
    {
        add(
            facts,
            FactKind::MaterialThreat { gain: proof.gain() },
            &proof.line,
        );
    }
}

fn existing_mate(
    before: &ChessState,
    from: crate::Square,
    follow: LegalMove,
    meter: &mut Meter,
) -> Option<bool> {
    for candidate in before
        .legal_moves()
        .into_iter()
        .filter(|candidate| candidate.from() == from && candidate.to() == follow.to())
    {
        meter.visit()?;
        let mut after = before.clone();
        meter.play(&mut after, candidate)?;
        if matches!(after.status(), GameStatus::Checkmate { .. }) {
            return Some(true);
        }
    }
    Some(false)
}

fn scores_better(analysis: &Analysis, mv: LegalMove) -> bool {
    let Some(best) = analysis.candidates.first() else {
        return false;
    };
    analysis
        .candidates
        .iter()
        .find(|candidate| candidate.pv.first() == Some(&mv))
        .is_some_and(|played| best.score != played.score)
}
