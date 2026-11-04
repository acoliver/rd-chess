//! Static exchange evaluation over actual legal captures on the arrival square.
//! Either player may decline further swaps. This is deliberately not a claim
//! that a recapture is forced, nor that an off-square zwischenzug is impossible.
use super::board::{material, victim};
use super::verify::{Meter, Proof, Value, better};
use super::{Fact, FactKind, add};
use crate::rules::{ChessState, LegalMove};
use crate::{Side, Square};

pub(super) fn see(state: &ChessState, mv: LegalMove, meter: &mut Meter, ply: u16) -> Option<Proof> {
    victim(state, mv)?;
    if meter.budget.depth == 0 {
        meter.unresolved = true;
        return None;
    }
    meter.visit()?;
    let side = state.side_to_move();
    let baseline = material(state, side);
    let mut after = state.clone();
    meter.play(&mut after, mv)?;
    let mut proof = swaps(&after, mv.to(), (side, baseline), meter, ply)?;
    proof.line.insert(0, mv);
    Some(proof)
}

fn swaps(
    state: &ChessState,
    target: Square,
    context: (Side, i32),
    meter: &mut Meter,
    ply: u16,
) -> Option<Proof> {
    meter.visit()?;
    let legal = state.legal_moves();
    let captures: Vec<_> = legal
        .iter()
        .copied()
        .filter(|&mv| mv.to() == target && victim(state, mv).is_some())
        .collect();
    if !captures.is_empty() && ply >= u16::from(meter.budget.depth.min(6)) + 8 {
        meter.unresolved = true;
        return None;
    }
    let forced = state.in_check() && !captures.is_empty() && captures.len() == legal.len();
    let mut best = (!forced).then(|| Proof {
        value: Value::Material(material(state, context.0) - context.1),
        line: Vec::new(),
    });
    for mv in captures {
        let mut after = state.clone();
        meter.play(&mut after, mv)?;
        let mut proof = swaps(&after, target, context, meter, ply + 1)?;
        proof.line.insert(0, mv);
        if better(best.as_ref(), &proof, state.side_to_move() == context.0) {
            best = Some(proof);
        }
    }
    best
}

pub(super) fn facts(
    state: &ChessState,
    after: &ChessState,
    mv: LegalMove,
    meter: &mut Meter,
    facts: &mut Vec<Fact>,
) {
    if let Some(proof) = see(state, mv, meter, 1) {
        add(
            facts,
            FactKind::Exchange { gain: proof.gain() },
            &proof.line,
        );
    }
    let mut captures: Vec<_> = after
        .legal_moves()
        .into_iter()
        .filter(|&reply| victim(after, reply).is_some())
        .collect();
    captures.sort_by_key(|reply| (reply.to() != mv.to(), reply.to(), reply.to_uci()));
    let mut reported = Vec::new();
    for reply in captures {
        if reported.contains(&reply.to()) {
            continue;
        }
        let Some(proof) = see(after, reply, meter, 2) else {
            continue;
        };
        if proof.gain() <= 0 {
            continue;
        }
        let (square, piece) = victim(after, reply).expect("capture has a victim");
        let mut line = vec![mv];
        line.extend(&proof.line);
        add(
            facts,
            FactKind::HangingPiece {
                piece,
                square,
                winner: after.side_to_move(),
                gain: proof.gain(),
            },
            &line,
        );
        reported.push(reply.to());
    }
}
