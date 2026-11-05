use super::evaluation::material;
use super::ordering::captured;
use super::score::Value;
use super::{Candidate, Limits, OpponentProfile, Score, Search, Style, evaluate};
use crate::rules::{ChessState, LegalMove};
use crate::{PieceKind, Side};
use std::cmp::Reverse;

/// Select from the profile's top-N safe search candidates, with deterministic
/// bounded centipawn perturbation. Never samples uniformly over legal moves.
/// An immediate enemy capture of any friendly queen excludes a candidate when
/// a queen-safe alternative exists. This conservative beginner safety rule may
/// reject sound queen sacrifices; deeper tactical strength is not claimed.
///
/// Requires at least one legal move (terminal mate/stalemate has none). On a
/// claimable draw with legal moves the host should claim rather than call this.
/// If no iteration completes, rank a finite one-ply static fallback instead;
/// that fallback and queen-safety preprocessing are not included in search nodes.
pub fn choose_move(state: &ChessState, profile: &OpponentProfile, seed: u64) -> LegalMove {
    choose_move_cancellable(state, profile, seed, &|| false)
        .expect("uncancelled policy has a legal move")
}

/// Cooperatively polls before policy candidates and every search node. On
/// cancellation returns the policy choice from the last completed iteration,
/// or None if none completed. Node-budget exhaustion still uses static fallback.
pub fn choose_move_cancellable(
    state: &ChessState,
    profile: &OpponentProfile,
    seed: u64,
    cancelled: &dyn Fn() -> bool,
) -> Option<LegalMove> {
    decide(state, profile, seed, cancelled).map(|decision| decision.chess_move)
}

/// Search work for the actual policy choice, not a separate analysis run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MoveDecision {
    pub chess_move: LegalMove,
    /// Visited search nodes; excludes finite policy preprocessing and fallback.
    pub nodes: u64,
    pub completed_depth: u8,
}

pub fn choose_move_measured(
    state: &ChessState,
    profile: &OpponentProfile,
    seed: u64,
) -> MoveDecision {
    decide(state, profile, seed, &|| false).expect("uncancelled policy has a legal move")
}

fn decide(
    state: &ChessState,
    profile: &OpponentProfile,
    seed: u64,
    cancelled: &dyn Fn() -> bool,
) -> Option<MoveDecision> {
    let legal = state.legal_moves();
    assert!(!legal.is_empty(), "choose_move requires a legal move");
    let mut safe = Vec::new();
    for &chess_move in &legal {
        if cancelled() {
            return None;
        }
        if queen_safe(state, chess_move) {
            safe.push(chess_move);
        }
    }
    let allowed = if safe.is_empty() { &legal } else { &safe };
    let analysis = Search::weighted(4096, profile.weights)
        .candidates(profile.candidates)
        .analyze_moves(
            state,
            Limits {
                nodes: profile.nodes,
                depth: Some(profile.depth),
            },
            cancelled,
            allowed,
        );
    let mut candidates = analysis.candidates;
    if candidates.is_empty() {
        if cancelled() {
            return None;
        }
        candidates = fallback(state, &legal, profile, cancelled)?;
    }
    candidates.retain(|candidate| allowed.contains(&candidate.pv[0]));
    candidates.sort_by_key(|candidate| Reverse(candidate.score.value(state.side_to_move())));
    let mut random = seed;
    Some(MoveDecision {
        chess_move: candidates
            .iter()
            .take(profile.candidates.max(1))
            .max_by_key(|candidate| policy_value(state, candidate, profile, &mut random))
            .expect("at least one allowed candidate")
            .pv[0],
        nodes: analysis.nodes,
        completed_depth: analysis.completed_depth,
    })
}

fn queen_safe(state: &ChessState, chess_move: LegalMove) -> bool {
    let mut next = state.clone();
    // Counter overflow is the rules API's only non-legality failure. Such an
    // imported position cannot advance; retain a legal move for host reporting.
    if super::play_generated(&mut next, chess_move).is_none() {
        return true;
    }
    !next
        .legal_moves()
        .into_iter()
        .any(|reply| next.piece_at(reply.to()) == Some((state.side_to_move(), PieceKind::Queen)))
}

fn fallback(
    state: &ChessState,
    legal: &[LegalMove],
    profile: &OpponentProfile,
    cancelled: &dyn Fn() -> bool,
) -> Option<Vec<Candidate>> {
    legal
        .iter()
        .map(|&chess_move| {
            if cancelled() {
                return None;
            }
            let mut next = state.clone();
            let score = if super::play_generated(&mut next, chess_move).is_some() {
                let value = evaluate(&next).total(profile.weights);
                Score::Centipawns(if state.side_to_move() == Side::Ivory {
                    value
                } else {
                    -value
                })
            } else {
                Score::Centipawns(0)
            };
            Some(Candidate {
                score,
                pv: vec![chess_move],
            })
        })
        .collect()
}

fn policy_value(
    state: &ChessState,
    candidate: &Candidate,
    profile: &OpponentProfile,
    random: &mut u64,
) -> Value {
    match candidate.score {
        Score::Centipawns(score) => {
            let amplitude = i32::from(profile.perturbation);
            let width = u64::try_from(amplitude * 2 + 1).expect("positive width");
            let noise =
                i32::try_from(next_random(random) % width).expect("bounded noise") - amplitude;
            Value::Pawns(score + noise + preference(state, candidate.pv[0], profile.style))
        }
        mate @ Score::Mate { .. } => mate.value(state.side_to_move()),
    }
}

fn next_random(random: &mut u64) -> u64 {
    *random = random.wrapping_add(0x9e37_79b9_7f4a_7c15);
    let mut value = *random;
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

fn preference(state: &ChessState, chess_move: LegalMove, style: Option<Style>) -> i32 {
    let kind = state.piece_at(chess_move.from()).expect("legal mover").1;
    match style {
        Some(Style::Development)
            if matches!(kind, PieceKind::Knight | PieceKind::Bishop)
                && [0, 7].contains(&chess_move.from().rank) =>
        {
            12
        }
        Some(Style::Initiative) if captured(state, chess_move).is_some() => 10,
        Some(Style::Exchanges)
            if captured(state, chess_move).is_some_and(|victim| {
                (material(victim).middlegame - material(kind).middlegame).abs() <= 50
            }) =>
        {
            10
        }
        Some(Style::Structure) if kind != PieceKind::Pawn => 6,
        _ => 0,
    }
}
