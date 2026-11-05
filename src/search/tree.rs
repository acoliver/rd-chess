use super::ordering::{Ordering as MoveOrdering, tactical};
use super::score::Value;
use super::table::{Bound, Entry, Probe, Table, context};
use super::{Analysis, Candidate, Limits, Line, Score, Weights, evaluate};
use crate::Side;
use crate::rules::{ChessState, GameStatus, LegalMove};
use std::cmp::Reverse;

#[derive(Clone, Copy)]
struct Window {
    alpha: Value,
    beta: Value,
}

impl Window {
    const FULL: Self = Self {
        alpha: Value::Bottom,
        beta: Value::Top,
    };
    fn reversed(self) -> Self {
        Self {
            alpha: self.beta.reversed(),
            beta: self.alpha.reversed(),
        }
    }
}

#[derive(Clone, Copy)]
struct Frame {
    depth: u8,
    ply: u16,
    window: Window,
}

pub(super) struct Context<'a> {
    table: &'a mut Table,
    weights: Weights,
    limits: Limits,
    cancel: &'a dyn Fn() -> bool,
    ordering: MoveOrdering,
    pub nodes: u64,
}

impl<'a> Context<'a> {
    pub fn new(
        table: &'a mut Table,
        weights: Weights,
        limits: Limits,
        cancel: &'a dyn Fn() -> bool,
    ) -> Self {
        Self {
            table,
            weights,
            limits,
            cancel,
            ordering: MoveOrdering::new(),
            nodes: 0,
        }
    }

    fn visit(&mut self) -> Option<()> {
        if self.nodes >= self.limits.nodes || (self.cancel)() {
            return None;
        }
        self.nodes += 1;
        Some(())
    }

    fn static_score(&self, state: &ChessState) -> Score {
        let value = evaluate(state).total(self.weights);
        Score::Centipawns(if state.side_to_move() == Side::Ivory {
            value
        } else {
            -value
        })
    }

    pub fn terminal(state: &ChessState, ply: u16) -> Option<Score> {
        Self::terminal_score(state.status(), ply)
    }

    fn terminal_score(status: GameStatus, ply: u16) -> Option<Score> {
        match status {
            GameStatus::Checkmate { winner } => Some(Score::Mate { winner, plies: ply }),
            GameStatus::Draw(_) => Some(Score::Centipawns(0)),
            GameStatus::Ongoing { claims, .. } if claims.fifty_move || claims.threefold => {
                Some(Score::Centipawns(0))
            }
            GameStatus::Ongoing { .. } => None,
        }
    }

    pub fn fallback(&self, state: &ChessState) -> Analysis {
        let terminal = Self::terminal(state, 0);
        Analysis {
            completed_depth: 0,
            nodes: 0,
            score: terminal.unwrap_or_else(|| self.static_score(state)),
            pv: if terminal.is_some() {
                Vec::new()
            } else {
                state.legal_moves().into_iter().take(1).collect()
            },
            candidates: Vec::new(),
        }
    }

    pub fn iteration(
        &mut self,
        state: &mut ChessState,
        depth: u8,
        width: usize,
        roots: Option<&[LegalMove]>,
    ) -> Option<Vec<Candidate>> {
        self.visit()?;
        let mut moves = state.legal_moves();
        if let Some(roots) = roots {
            moves.retain(|chess_move| roots.contains(chess_move));
        }
        self.ordering
            .sort(state, &mut moves, self.table.best_move(state), 0);
        let mut candidates: Vec<Candidate> = Vec::with_capacity(width.min(moves.len()));
        for chess_move in moves {
            let alpha = if candidates.len() == width {
                candidates
                    .last()
                    .expect("nonzero candidate width")
                    .score
                    .value(state.side_to_move())
            } else {
                Value::Bottom
            };
            let window = Window {
                alpha,
                beta: Value::Top,
            };
            let frame = Frame {
                depth: depth - 1,
                ply: 1,
                window: window.reversed(),
            };
            let line = self.child(state, chess_move, frame)?;
            // A fail-low root cannot enter the exact top-N set. Every admitted
            // root is inside its window, so its score is exact rather than a bound.
            if line.score.value(state.side_to_move()) > alpha {
                candidates.push(Candidate {
                    score: line.score,
                    pv: line.pv,
                });
                candidates
                    .sort_by_key(|candidate| Reverse(candidate.score.value(state.side_to_move())));
                candidates.truncate(width);
            }
        }
        let best = candidates.first().expect("at least one legal root");
        let line = Line {
            score: best.score,
            pv: best.pv.clone(),
        };
        self.store(
            state,
            Frame {
                depth,
                ply: 0,
                window: Window::FULL,
            },
            &line,
        );
        Some(candidates)
    }

    fn child(
        &mut self,
        state: &mut ChessState,
        chess_move: LegalMove,
        frame: Frame,
    ) -> Option<Line> {
        super::play_generated(state, chess_move)?;
        let result = self.node(state, frame);
        state.undo().expect("search move has an undo snapshot");
        result.map(|line| {
            let mut pv = vec![chess_move];
            pv.extend(line.pv);
            Line {
                score: line.score.reversed(),
                pv,
            }
        })
    }

    fn node(&mut self, state: &mut ChessState, frame: Frame) -> Option<Line> {
        self.visit()?;
        let moves = state.legal_moves();
        if let Some(score) = Self::terminal_score(state.status_with_legal(&moves), frame.ply) {
            return Some(Line {
                score,
                pv: Vec::new(),
            });
        }
        if frame.ply >= 128 {
            return None;
        }
        if frame.depth > 0
            && let Some(line) = self.table.hit(
                state,
                Probe {
                    depth: frame.depth,
                    ply: frame.ply,
                    side: state.side_to_move(),
                    alpha: frame.window.alpha,
                    beta: frame.window.beta,
                },
            )
        {
            return Some(line);
        }
        let line = self.expand(state, frame, moves)?;
        if frame.depth > 0 {
            self.store(state, frame, &line);
        }
        Some(line)
    }

    fn expand(
        &mut self,
        state: &mut ChessState,
        frame: Frame,
        mut moves: Vec<LegalMove>,
    ) -> Option<Line> {
        let side = state.side_to_move();
        let quiet = frame.depth == 0 && !state.in_check();
        let mut best = quiet.then(|| Line {
            score: self.static_score(state),
            pv: Vec::new(),
        });
        let mut window = frame.window;
        if let Some(line) = &best {
            window.alpha = window.alpha.max(line.score.value(side));
            if window.alpha >= window.beta {
                return best;
            }
        }
        if quiet {
            moves.retain(|&chess_move| tactical(state, chess_move));
        }
        self.ordering
            .sort(state, &mut moves, self.table.best_move(state), frame.ply);
        for chess_move in moves {
            let child = Frame {
                depth: frame.depth.saturating_sub(1),
                ply: frame.ply + 1,
                window: window.reversed(),
            };
            let line = self.child(state, chess_move, child)?;
            if best
                .as_ref()
                .is_none_or(|old| line.score.value(side) > old.score.value(side))
            {
                best = Some(line);
            }
            window.alpha = window
                .alpha
                .max(best.as_ref().expect("visited move").score.value(side));
            if window.alpha >= window.beta {
                self.ordering
                    .cutoff(state, chess_move, frame.ply, frame.depth);
                break;
            }
        }
        best
    }

    fn store(&mut self, state: &ChessState, frame: Frame, line: &Line) {
        let value = line.score.value(state.side_to_move());
        let bound = if value <= frame.window.alpha {
            Bound::Upper
        } else if value >= frame.window.beta {
            Bound::Lower
        } else {
            Bound::Exact
        };
        self.table.put(
            Entry {
                hash: state.position_hash(),
                context: context(state),
                depth: frame.depth,
                age: self.table.age,
                bound,
                line: line.clone(),
            },
            frame.ply,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quiescence(fen: &str) -> Line {
        let mut state = ChessState::from_fen(fen).unwrap();
        let mut table = Table::new(0);
        let cancel = || false;
        let mut context = Context::new(
            &mut table,
            Weights::BALANCED,
            Limits {
                nodes: 2000,
                depth: Some(0),
            },
            &cancel,
        );
        context
            .node(
                &mut state,
                Frame {
                    depth: 0,
                    ply: 0,
                    window: Window::FULL,
                },
            )
            .unwrap()
    }

    #[test]
    fn quiescence_searches_check_evasions_without_stand_pat() {
        let line = quiescence("4k3/8/8/8/8/8/4r3/4K3 w - - 0 1");
        assert_eq!(line.score, Score::Centipawns(0));
        assert_eq!(line.pv[0].to_uci(), "e1e2");
        let line = quiescence("4r1k1/8/8/8/1b6/8/8/R3K3 w - - 0 1");
        assert!(!line.pv.is_empty());
    }

    #[test]
    fn quiescence_includes_non_capture_promotions_and_en_passant() {
        let line = quiescence("7k/P7/8/8/8/8/8/4K3 w - - 0 1");
        assert!(line.pv[0].promotion().is_some());
        assert!(line.score.value(Side::Ivory) > Value::Pawns(800));
        let line = quiescence("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 2");
        assert_eq!(line.pv[0].to_uci(), "e5d6");
    }
}
