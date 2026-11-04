use super::BoardDriver;
use crate::{
    Side,
    rules::{ChessState, GameStatus, LegalMove},
    search::{LADDER, choose_move_cancellable},
};
use std::{
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
    time::{Duration, Instant},
};
use worldrunner::scene::viewer::{RetainedHost, WorkQueue, WorkSubmitError, WorkTicket};

const SEARCH_BUDGET: Duration = Duration::from_secs(10);

/// Game setup shared by CLI and new-game menus. Levels are 1 through 8.
#[derive(Clone, Copy, Debug)]
pub struct ComputerGame {
    human: Side,
    level: u8,
    seed: u64,
}
impl ComputerGame {
    pub fn human(self) -> Side {
        self.human
    }
    pub fn level(self) -> u8 {
        self.level
    }
    pub fn seed(self) -> u64 {
        self.seed
    }

    pub fn new(human: Side, level: u8, seed: u64) -> Result<Self, String> {
        if !(1..=8).contains(&level) {
            return Err("computer level must be 1..=8".into());
        }
        Ok(Self { human, level, seed })
    }
}

pub(super) struct Computer {
    config: ComputerGame,
    generation: u64,
    queue: Option<WorkQueue<Reply>>,
    pending: Option<Request>,
    message: Option<String>,
}
struct Request {
    ticket: WorkTicket<Reply>,
    generation: u64,
    deadline: Instant,
    stop: Arc<AtomicBool>,
}
impl Request {
    #[cfg(test)]
    fn new(ticket: WorkTicket<Reply>, generation: u64) -> Self {
        Self {
            ticket,
            generation,
            deadline: Instant::now() + SEARCH_BUDGET,
            stop: Arc::new(AtomicBool::new(false)),
        }
    }
}
struct Reply {
    generation: u64,
    chess_move: Option<LegalMove>,
}
impl Computer {
    pub(super) fn changed(&mut self) {
        if let Some(request) = self.pending.take() {
            self.queue
                .as_ref()
                .expect("submitted queue")
                .cancel(&request.ticket);
        }
        self.generation = self
            .generation
            .checked_add(1)
            .expect("game generation exhausted");
        self.message = None;
    }
    fn submit(&mut self, state: ChessState) {
        let generation = self.generation;
        let config = self.config;
        let deadline = Instant::now() + SEARCH_BUDGET;
        let stop = Arc::new(AtomicBool::new(false));
        let worker_stop = stop.clone();
        let seed = move_seed(config.seed, state.repetition_history().len() - 1);
        let result = self
            .queue
            .as_ref()
            .expect("initialized queue")
            .submit(move |cancel| {
                let chess_move = choose_move_cancellable(
                    &state,
                    &LADDER[usize::from(config.level - 1)],
                    seed,
                    &|| {
                        cancel.is_cancelled()
                            || worker_stop.load(Ordering::Relaxed)
                            || Instant::now() >= deadline
                    },
                );
                Reply {
                    generation,
                    chess_move,
                }
            });
        match result {
            Ok(ticket) => {
                self.pending = Some(Request {
                    ticket,
                    generation,
                    deadline,
                    stop,
                });
            }
            Err(WorkSubmitError::Full) => (),
            Err(error) => self.message = Some(format!("Computer error: {error}")),
        }
    }
}

impl BoardDriver {
    pub fn computer_game(&self) -> Option<ComputerGame> {
        self.computer.as_ref().map(|computer| computer.config)
    }

    pub fn set_computer(&mut self, config: ComputerGame) -> Result<(), String> {
        if self.board.is_none() {
            return Err("editor scenes have no legal game".into());
        }
        if let Some(computer) = &mut self.computer {
            computer.changed();
        }
        self.clear_selection()?;
        self.computer = Some(Computer {
            config,
            generation: 0,
            queue: None,
            pending: None,
            message: None,
        });
        Ok(())
    }
    pub(super) fn computer_turn(&self) -> bool {
        self.computer.as_ref().is_some_and(|computer| {
            self.board
                .as_ref()
                .expect("computer game")
                .game()
                .side_to_move()
                != computer.config.human
        })
    }
    pub(super) fn computer_status(&self) -> Option<String> {
        let computer = self.computer.as_ref()?;
        if let Some(message) = &computer.message {
            return Some(message.clone());
        }
        (self.computer_turn()
            && matches!(
                self.board.as_ref()?.game().status(),
                GameStatus::Ongoing { .. }
            ))
        .then(|| {
            format!(
                "Computer (Level {}, {}) is thinking",
                computer.config.level,
                LADDER[usize::from(computer.config.level - 1)].name
            )
        })
    }
    pub(super) fn advance_computer(&mut self, host: &mut RetainedHost) {
        if self.screen() != super::Screen::Play || self.game_ended() || !self.computer_turn() {
            return;
        }
        if let Some(reason) = self.draw_claim_reason() {
            self.finish(format!("Draw: {reason} claimed by computer"))
                .expect("valid game UI");
            return;
        }
        let state = self.board.as_ref().expect("computer board").game();
        let computer = self.computer.as_mut().expect("computer turn");
        if let Some(request) = &computer.pending {
            if Instant::now() >= request.deadline {
                request.stop.store(true, Ordering::Relaxed);
            }
            return;
        }
        if computer.message.is_some() {
            return;
        }
        if computer.queue.is_none() {
            match host.work_queue(1, 1) {
                Ok(queue) => computer.queue = Some(queue),
                Err(error) => {
                    computer.message = Some(format!("Computer error: {error}"));
                    return;
                }
            }
        }
        computer.submit(state.clone());
    }
    pub(super) fn drain_computer(&mut self) -> Result<(), String> {
        loop {
            let completion = self
                .computer
                .as_ref()
                .and_then(|c| c.queue.as_ref())
                .and_then(WorkQueue::try_recv);
            let Some(completion) = completion else {
                return Ok(());
            };
            let computer = self.computer.as_mut().expect("queue owner");
            let Some(request) = &computer.pending else {
                continue;
            };
            if completion.ticket != request.ticket || request.generation != computer.generation {
                continue;
            }
            let generation = request.generation;
            computer.pending = None;
            match completion.result {
                Ok(reply) => self.accept_reply(reply.generation, reply.chess_move)?,
                Err(error) => {
                    self.computer.as_mut().expect("queue owner").message =
                        Some(format!("Computer error: {error} (request {generation})"));
                }
            }
        }
    }
    fn accept_reply(
        &mut self,
        generation: u64,
        chess_move: Option<LegalMove>,
    ) -> Result<(), String> {
        let computer = self.computer.as_mut().expect("reply owner");
        if generation != computer.generation {
            return Ok(());
        }
        let Some(chess_move) = chess_move else {
            computer.message =
                Some("Computer error: search timed out before completing an iteration".into());
            return Ok(());
        };
        let board = self.board.as_ref().expect("computer board");
        if !self.computer_turn() || !board.game().legal_moves().contains(&chess_move) {
            self.computer.as_mut().expect("reply owner").message =
                Some("Computer error: illegal search result".into());
            return Ok(());
        }
        self.commit_move(chess_move)
    }
    pub(super) fn undo_plies(&self) -> Result<usize, String> {
        let board = self
            .board
            .as_ref()
            .ok_or("editor scenes have no legal game")?;
        let plies = board.game().repetition_history().len() - 1;
        let count = self.computer.as_ref().map_or(1, |computer| {
            if board.game().side_to_move() == computer.config.human {
                2
            } else {
                1
            }
        });
        if plies < count {
            return Err("no human move to undo".into());
        }
        Ok(count)
    }
}

pub(super) fn move_seed(seed: u64, ply: usize) -> u64 {
    let mut value = seed.wrapping_add((ply as u64).wrapping_mul(0x9e37_79b9_7f4a_7c15));
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

#[cfg(test)]
mod tests;
