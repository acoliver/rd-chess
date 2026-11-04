use super::Line;
use super::score::Value;
use crate::Side;
use crate::rules::{ChessState, LegalMove};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Bound {
    Exact,
    Lower,
    Upper,
}

#[derive(Clone)]
pub(super) struct Entry {
    pub hash: u64,
    pub context: u64,
    pub depth: u8,
    pub age: u32,
    pub bound: Bound,
    pub line: Line,
}

pub(super) struct Table {
    slots: Vec<Option<Entry>>,
    pub age: u32,
}

/// The Zobrist key omits draw context. Validate a second fingerprint of the
/// reversible history multiset and halfmove clock before reusing bounds. Order
/// is irrelevant to repetition counts, but multiplicity is retained by addition.
/// As with the position key itself, hash collisions remain theoretically possible.
pub(super) fn context(state: &ChessState) -> u64 {
    let history = state.repetition_history();
    let span = usize::try_from(state.halfmove_clock())
        .expect("u32 fits usize")
        .saturating_add(1);
    history[history.len().saturating_sub(span)..].iter().fold(
        u64::from(state.halfmove_clock()).wrapping_mul(0x9e37_79b9_7f4a_7c15),
        |sum, hash| sum.wrapping_add(hash.wrapping_mul(0xbf58_476d_1ce4_e5b9).rotate_left(23)),
    )
}

impl Table {
    pub fn new(capacity: usize) -> Self {
        Self {
            slots: vec![None; capacity],
            age: 0,
        }
    }

    fn offset(&self, hash: u64) -> Option<usize> {
        let capacity = u64::try_from(self.slots.len()).expect("capacity fits u64");
        (capacity != 0).then(|| usize::try_from(hash % capacity).expect("slot fits usize"))
    }

    pub fn get(&self, state: &ChessState) -> Option<&Entry> {
        let entry = self.slots[self.offset(state.position_hash())?].as_ref()?;
        (entry.hash == state.position_hash()).then_some(entry)
    }

    pub fn best_move(&self, state: &ChessState) -> Option<LegalMove> {
        self.get(state)?.line.pv.first().copied()
    }

    pub fn hit(&self, state: &ChessState, probe: Probe) -> Option<Line> {
        let entry = self.get(state)?;
        if entry.depth < probe.depth
            || entry.context != context(state)
            || entry.line.pv.len() + usize::from(probe.ply) > 128
        {
            return None;
        }
        let score = entry.line.score.restored(probe.ply);
        let value = score.value(probe.side);
        let usable = match entry.bound {
            Bound::Exact => true,
            Bound::Lower => value >= probe.beta,
            Bound::Upper => value <= probe.alpha,
        };
        usable.then(|| Line {
            score,
            pv: entry.line.pv.clone(),
        })
    }

    pub fn put(&mut self, mut entry: Entry, ply: u16) {
        let Some(offset) = self.offset(entry.hash) else {
            return;
        };
        let replace = self.slots[offset]
            .as_ref()
            .is_none_or(|old| old.age != entry.age || entry.depth >= old.depth);
        if replace {
            entry.line.score = entry.line.score.stored(ply);
            self.slots[offset] = Some(entry);
        }
    }
}

#[derive(Clone, Copy)]
pub(super) struct Probe {
    pub depth: u8,
    pub ply: u16,
    pub side: Side,
    pub alpha: Value,
    pub beta: Value,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Score;

    #[test]
    fn mate_distance_adjusts_through_actual_table_hits() {
        let state = ChessState::initial();
        for winner in [Side::Ivory, Side::Onyx] {
            let mut table = Table::new(1);
            table.put(
                Entry {
                    hash: state.position_hash(),
                    context: context(&state),
                    depth: 4,
                    age: 0,
                    bound: Bound::Exact,
                    line: Line {
                        score: Score::Mate { winner, plies: 9 },
                        pv: vec![state.legal_moves()[0]],
                    },
                },
                6,
            );
            let hit = table
                .hit(
                    &state,
                    Probe {
                        depth: 3,
                        ply: 2,
                        side: Side::Ivory,
                        alpha: Value::Bottom,
                        beta: Value::Top,
                    },
                )
                .unwrap();
            assert_eq!(hit.score, Score::Mate { winner, plies: 5 });
        }
    }

    #[test]
    fn equal_positions_and_clocks_with_distinct_reversible_histories_do_not_share_bounds() {
        let mut left = ChessState::initial();
        let mut right = ChessState::initial();
        for (state, cycle) in [
            (&mut left, ["g1f3", "g8f6", "f3g1", "f6g8"]),
            (&mut right, ["b1c3", "b8c6", "c3b1", "c6b8"]),
        ] {
            for uci in cycle {
                state.play(state.parse_uci(uci).unwrap()).unwrap();
            }
        }
        assert_eq!(left.position_hash(), right.position_hash());
        assert_eq!(left.halfmove_clock(), right.halfmove_clock());
        assert_ne!(context(&left), context(&right));
        let mut table = Table::new(1);
        table.put(
            Entry {
                hash: left.position_hash(),
                context: context(&left),
                depth: 2,
                age: 0,
                bound: Bound::Exact,
                line: Line {
                    score: Score::Centipawns(200),
                    pv: Vec::new(),
                },
            },
            0,
        );
        assert!(
            table
                .hit(
                    &right,
                    Probe {
                        depth: 1,
                        ply: 0,
                        side: Side::Ivory,
                        alpha: Value::Bottom,
                        beta: Value::Top,
                    }
                )
                .is_none()
        );
    }

    #[test]
    fn bounds_require_draw_history_and_clock_agreement() {
        let state = ChessState::initial();
        let mut table = Table::new(1);
        table.put(
            Entry {
                hash: state.position_hash(),
                context: context(&state),
                depth: 2,
                age: 0,
                bound: Bound::Exact,
                line: Line {
                    score: Score::Centipawns(200),
                    pv: Vec::new(),
                },
            },
            0,
        );
        let later =
            ChessState::from_fen("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 99 80")
                .unwrap();
        assert_eq!(state.position_hash(), later.position_hash());
        assert!(
            table
                .hit(
                    &later,
                    Probe {
                        depth: 1,
                        ply: 0,
                        side: Side::Ivory,
                        alpha: Value::Bottom,
                        beta: Value::Top,
                    }
                )
                .is_none()
        );
    }
}
