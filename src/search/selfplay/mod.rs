//! CPU self-play of the production opponent policy. Each opening/seed is played
//! twice with reversed colours. Claims are taken immediately, automatic rules
//! outcomes take precedence, and the ply cap is a draw (no evaluation adjudication).
//! Opening UCI lines are original short selections of standard opening moves.
//! Worker scheduling changes timings only, never seeds, game order or choices.

use super::{OpponentProfile, choose_move_measured};
use crate::Side;
use crate::rules::{ChessState, GameStatus, LegalMove};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

mod report;
pub use report::MatchReport;

const OPENINGS: &str = include_str!("openings.txt");

#[derive(Clone, Copy, Debug)]
pub struct MatchConfig {
    pub games: u32,
    pub threads: usize,
    /// Searched plies after the opening, not full moves.
    pub max_plies: u32,
    pub seed: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Finish {
    Mate(Side),
    RulesDraw,
    ClaimDraw,
    MoveCap,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct MoveStats {
    pub moves: u32,
    pub nodes: u64,
    pub elapsed: Duration,
    pub longest: Duration,
}

impl MoveStats {
    pub fn merge(&mut self, other: Self) {
        self.moves += other.moves;
        self.nodes += other.nodes;
        self.elapsed += other.elapsed;
        self.longest = self.longest.max(other.longest);
    }
}

#[derive(Clone, Debug)]
pub struct Game {
    pub index: u32,
    pub moves: Vec<LegalMove>,
    pub finish: Finish,
    /// Profile order, independent of colour. Includes all policy preprocessing.
    pub stats: [MoveStats; 2],
}

/// Games must be paired, workers are restricted to 1..=6, and zero caps fail.
/// Worker panics propagate; they are never converted into scored losses.
pub fn play_match(
    profiles: [OpponentProfile; 2],
    config: MatchConfig,
) -> Result<Vec<Game>, &'static str> {
    if config.games == 0 || !config.games.is_multiple_of(2) {
        return Err("games must be positive and even for colour pairing");
    }
    if !(1..=6).contains(&config.threads) || config.max_plies == 0 {
        return Err("threads must be 1..=6 and max plies must be positive");
    }
    let next = AtomicU32::new(0);
    let openings = opening_positions();
    let mut games = std::thread::scope(|scope| {
        let workers: Vec<_> = (0..config.threads)
            .map(|_| scope.spawn(|| worker(profiles, config, &next, &openings)))
            .collect();
        workers
            .into_iter()
            .flat_map(|worker| worker.join().expect("self-play worker panicked"))
            .collect::<Vec<_>>()
    });
    games.sort_by_key(|game| game.index);
    Ok(games)
}

fn opening(line: &str) -> ChessState {
    let mut state = ChessState::initial();
    for uci in line.split_whitespace() {
        state
            .play(state.parse_uci(uci).expect("legal opening UCI"))
            .expect("opening applies");
    }
    state
}
/// The same fixed positions used by paired games, with opening history intact.
pub fn opening_positions() -> Vec<ChessState> {
    OPENINGS.lines().map(opening).collect()
}

fn worker(
    profiles: [OpponentProfile; 2],
    config: MatchConfig,
    next: &AtomicU32,
    openings: &[ChessState],
) -> Vec<Game> {
    let mut games = Vec::new();
    loop {
        let index = next.fetch_add(1, Ordering::Relaxed);
        if index >= config.games {
            break;
        }
        let pair = usize::try_from(index / 2).expect("game index fits usize");
        games.push(play_game(
            profiles,
            config,
            index,
            openings[pair % openings.len()].clone(),
        ));
    }
    games
}

fn play_game(
    profiles: [OpponentProfile; 2],
    config: MatchConfig,
    index: u32,
    mut state: ChessState,
) -> Game {
    let mut game = Game {
        index,
        moves: Vec::new(),
        finish: Finish::MoveCap,
        stats: [MoveStats::default(); 2],
    };
    let pair_seed = mix(config.seed.wrapping_add(u64::from(index / 2)));
    for ply in 0..config.max_plies {
        if let Some(finish) = outcome(&state) {
            game.finish = finish;
            return game;
        }
        let player = usize::from(state.side_to_move() == Side::Onyx)
            ^ usize::try_from(index % 2).expect("parity");
        let start = Instant::now();
        let decision = choose_move_measured(
            &state,
            &profiles[player],
            mix(pair_seed.wrapping_add(u64::from(ply))),
        );
        let elapsed = start.elapsed();
        game.stats[player].merge(MoveStats {
            moves: 1,
            nodes: decision.nodes,
            elapsed,
            longest: elapsed,
        });
        state
            .play(decision.chess_move)
            .expect("production policy move must be legal");
        game.moves.push(decision.chess_move);
    }
    game.finish = outcome(&state).unwrap_or(Finish::MoveCap);
    game
}

fn outcome(state: &ChessState) -> Option<Finish> {
    match state.status() {
        GameStatus::Checkmate { winner } => Some(Finish::Mate(winner)),
        GameStatus::Draw(_) => Some(Finish::RulesDraw),
        GameStatus::Ongoing { claims, .. } if claims.fifty_move || claims.threefold => {
            Some(Finish::ClaimDraw)
        }
        GameStatus::Ongoing { .. } => None,
    }
}

fn mix(mut value: u64) -> u64 {
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests;
