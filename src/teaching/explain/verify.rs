use super::Budget;
use super::board::{material, value, victim};
use crate::Side;
use crate::rules::{ChessState, GameStatus, LegalMove};
use std::cmp::Reverse;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum Value {
    Bottom,
    Loss(u16),
    Material(i32),
    Win(Reverse<u16>),
    Top,
}

#[derive(Clone, Debug)]
pub(super) struct Proof {
    pub value: Value,
    pub line: Vec<LegalMove>,
}

impl Proof {
    pub fn gain(&self) -> i32 {
        if let Value::Material(gain) = self.value {
            gain
        } else {
            0
        }
    }
    pub fn wins_mate(&self) -> bool {
        matches!(self.value, Value::Win(_))
    }
}

pub(super) struct Meter {
    pub budget: Budget,
    pub nodes: u64,
    pub unresolved: bool,
}

impl Meter {
    pub fn visit(&mut self) -> Option<()> {
        if self.nodes >= self.budget.nodes {
            self.unresolved = true;
            return None;
        }
        self.nodes += 1;
        Some(())
    }

    pub fn play(&mut self, state: &mut ChessState, mv: LegalMove) -> Option<()> {
        match state.play(mv) {
            Ok(()) => Some(()),
            Err(crate::rules::MoveError::CounterOverflow) => {
                self.unresolved = true;
                None
            }
            Err(error) => panic!("generated verification move rejected: {error}"),
        }
    }
}

#[derive(Clone, Copy)]
struct Frame {
    depth: u8,
    ply: u16,
    alpha: Value,
    beta: Value,
}

struct Tree<'a> {
    meter: &'a mut Meter,
    side: Side,
    baseline: i32,
}

pub(super) fn prove(state: &ChessState, mv: LegalMove, meter: &mut Meter) -> Option<Proof> {
    let depth = meter.budget.depth.min(6);
    if depth == 0 {
        meter.unresolved = true;
        return None;
    }
    meter.visit()?;
    let mut after = state.clone();
    meter.play(&mut after, mv)?;
    let mut tree = Tree {
        meter,
        side: state.side_to_move(),
        baseline: material(state, state.side_to_move()),
    };
    let mut proof = tree.node(
        &after,
        Frame {
            depth: depth - 1,
            ply: 1,
            alpha: Value::Bottom,
            beta: Value::Top,
        },
    )?;
    proof.line.insert(0, mv);
    Some(proof)
}

impl Tree<'_> {
    fn terminal(&self, state: &ChessState, ply: u16) -> Option<Proof> {
        let value = match state.status() {
            GameStatus::Checkmate { winner } if winner == self.side => Value::Win(Reverse(ply)),
            GameStatus::Checkmate { .. } => Value::Loss(ply),
            // A drawn outcome does not undo captures. This verifier measures
            // material changes, unlike the game-outcome search used for ranking.
            GameStatus::Draw(_) => Value::Material(material(state, self.side) - self.baseline),
            GameStatus::Ongoing { claims, .. } if claims.fifty_move || claims.threefold => {
                Value::Material(material(state, self.side) - self.baseline)
            }
            GameStatus::Ongoing { .. } => return None,
        };
        Some(Proof {
            value,
            line: Vec::new(),
        })
    }

    fn node(&mut self, state: &ChessState, mut frame: Frame) -> Option<Proof> {
        self.meter.visit()?;
        if let Some(proof) = self.terminal(state, frame.ply) {
            return Some(proof);
        }
        let quiet = frame.depth == 0 && !state.in_check();
        let maximize = state.side_to_move() == self.side;
        let mut best = quiet.then(|| Proof {
            value: Value::Material(material(state, self.side) - self.baseline),
            line: Vec::new(),
        });
        if let Some(proof) = &best {
            bound(&mut frame, proof.value, maximize);
            if frame.alpha >= frame.beta {
                return best;
            }
        }
        let moves = ordered(state, quiet);
        if !moves.is_empty() && frame.ply >= u16::from(self.meter.budget.depth.min(6)) + 8 {
            self.meter.unresolved = true;
            return None;
        }
        for mv in moves {
            let mut after = state.clone();
            self.meter.play(&mut after, mv)?;
            let child = Frame {
                depth: frame.depth.saturating_sub(1),
                ply: frame.ply + 1,
                ..frame
            };
            let mut proof = self.node(&after, child)?;
            proof.line.insert(0, mv);
            if better(best.as_ref(), &proof, maximize) {
                best = Some(proof);
            }
            bound(
                &mut frame,
                best.as_ref().expect("visited move").value,
                maximize,
            );
            if frame.alpha >= frame.beta {
                break;
            }
        }
        best
    }
}

fn bound(frame: &mut Frame, value: Value, maximize: bool) {
    if maximize {
        frame.alpha = frame.alpha.max(value);
    } else {
        frame.beta = frame.beta.min(value);
    }
}

fn ordered(state: &ChessState, quiet: bool) -> Vec<LegalMove> {
    let mut moves = state.legal_moves();
    if quiet {
        moves.retain(|&mv| victim(state, mv).is_some() || mv.promotion().is_some());
    }
    moves.sort_by_key(|&mv| {
        Reverse((
            mv.promotion().is_some(),
            victim(state, mv).map_or(0, |(_, kind)| value(kind)),
        ))
    });
    moves
}

pub(super) fn better(old: Option<&Proof>, new: &Proof, maximize: bool) -> bool {
    old.is_none_or(|old| {
        if maximize {
            new.value > old.value
        } else {
            new.value < old.value
        }
    })
}
