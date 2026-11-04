use super::board::{attacks, captured_in, direction, material, shifted, slider, squares, value};
use super::verify::Proof;
use super::{Fact, FactKind, add};
use crate::rules::{ChessState, LegalMove};
use crate::{PieceKind, Square};

const RAYS: [(i16, i16); 8] = [
    (1, 0),
    (-1, 0),
    (0, 1),
    (0, -1),
    (1, 1),
    (1, -1),
    (-1, 1),
    (-1, -1),
];

pub(super) fn facts(
    before: &ChessState,
    after: &ChessState,
    mv: LegalMove,
    proof: Option<&Proof>,
    facts: &mut Vec<Fact>,
) {
    let side = before.side_to_move();
    let immediate_gain = material(after, side) - material(before, side);
    let proof = proof.filter(|proof| proof.gain() > immediate_gain.max(0));
    discovered(before, after, mv, proof, facts);
    let Some(proof) = proof else {
        return;
    };
    fork(after, mv, proof, facts);
    for attacker in squares().filter(|&sq| {
        after
            .piece_at(sq)
            .is_some_and(|(side, _)| side == before.side_to_move())
    }) {
        for step in RAYS {
            line_motif(before, after, attacker, step, proof, facts);
        }
    }
}

fn fork(after: &ChessState, mv: LegalMove, proof: &Proof, facts: &mut Vec<Fact>) {
    let (side, kind) = after.piece_at(mv.to()).expect("moved piece");
    let targets: Vec<_> = squares()
        .filter(|&sq| {
            after.piece_at(sq).is_some_and(|(owner, target)| {
                owner != side
                    && (target == PieceKind::King
                        || value(target) > value(kind)
                        || !defended(after, sq, owner))
            }) && attacks(after, mv.to(), sq)
        })
        .collect();
    if targets.len() >= 2
        && targets
            .iter()
            .any(|&target| captured_in(after, &proof.line[1..], mv.to(), target))
    {
        add(
            facts,
            FactKind::Fork {
                attacker: mv.to(),
                targets,
            },
            &proof.line,
        );
    }
}

fn defended(state: &ChessState, target: Square, side: crate::Side) -> bool {
    squares().any(|from| {
        state.piece_at(from).is_some_and(|(owner, _)| owner == side) && attacks(state, from, target)
    })
}

fn next_man(state: &ChessState, from: Square, step: (i16, i16)) -> Option<Square> {
    let mut cursor = shifted(from, step);
    while let Some(sq) = cursor {
        if state.piece_at(sq).is_some() {
            return Some(sq);
        }
        cursor = shifted(sq, step);
    }
    None
}

fn line_motif(
    before: &ChessState,
    after: &ChessState,
    attacker: Square,
    step: (i16, i16),
    proof: &Proof,
    facts: &mut Vec<Fact>,
) {
    let (side, kind) = after.piece_at(attacker).expect("candidate attacker");
    if !slider(kind, step) {
        return;
    }
    let Some(front) = next_man(after, attacker, step) else {
        return;
    };
    let Some(rear) = next_man(after, front, step) else {
        return;
    };
    let (front_side, front_piece) = after.piece_at(front).expect("front piece");
    let (rear_side, rear_piece) = after.piece_at(rear).expect("rear piece");
    if front_side == side || rear_side == side || attacks(before, attacker, front) {
        return;
    }
    let line = &proof.line[1..];
    let kind = if (rear_piece == PieceKind::King || value(rear_piece) > value(front_piece))
        && captured_in(after, line, attacker, front)
    {
        Some(FactKind::Pin {
            attacker,
            front,
            rear,
            rear_piece,
        })
    } else if (front_piece == PieceKind::King || value(front_piece) > value(rear_piece))
        && captured_in(after, line, attacker, rear)
    {
        Some(FactKind::Skewer {
            attacker,
            front,
            rear,
        })
    } else {
        None
    };
    if let Some(kind) = kind {
        add(facts, kind, &proof.line);
    }
}

fn discovered(
    before: &ChessState,
    after: &ChessState,
    mv: LegalMove,
    proof: Option<&Proof>,
    facts: &mut Vec<Fact>,
) {
    for attacker in
        squares().filter(|&sq| sq != mv.to() && before.piece_at(sq) == after.piece_at(sq))
    {
        let Some((side, piece)) = after.piece_at(attacker) else {
            continue;
        };
        if side != before.side_to_move() {
            continue;
        }
        for target in
            squares().filter(|&sq| after.piece_at(sq).is_some_and(|(owner, _)| owner != side))
        {
            if !slider(piece, direction(attacker, target))
                || !attacks(after, attacker, target)
                || attacks(before, attacker, target)
            {
                continue;
            }
            if after
                .piece_at(target)
                .is_some_and(|(_, kind)| kind == PieceKind::King)
            {
                add(facts, FactKind::DiscoveredCheck { attacker, piece }, &[mv]);
            } else if let Some(proof) = proof.filter(|proof| {
                proof.gain() > 0 && captured_in(after, &proof.line[1..], attacker, target)
            }) {
                add(
                    facts,
                    FactKind::DiscoveredAttack {
                        attacker,
                        target,
                        piece,
                    },
                    &proof.line,
                );
            }
        }
    }
}
