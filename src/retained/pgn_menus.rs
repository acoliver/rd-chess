use super::{
    BoardDriver, Screen,
    menus::MenuAction,
    persistence::{GamePayload, SavedMode},
    pgn_files,
};
use crate::{
    Side,
    rules::{
        ChessState, GameStatus,
        pgn::{PgnGame, PgnResult},
    },
};
use std::path::Path;

#[derive(Default)]
pub(super) struct PgnMenu {
    files: Vec<String>,
    games: Vec<PgnGame>,
    pub message: Option<String>,
}

impl BoardDriver {
    fn pgn_root(&self) -> Result<&Path, String> {
        self.storage
            .as_ref()
            .map(|s| s.root.as_path())
            .ok_or_else(|| "no application store".into())
    }
    pub fn pgn_files(&self) -> &[String] {
        &self.pgn.files
    }
    pub fn pgn_game_labels(&self) -> Vec<String> {
        self.pgn
            .games
            .iter()
            .enumerate()
            .map(|(index, game)| {
                let player = |tag: &str| game.tags.get(tag).map_or("?", String::as_str);
                format!(
                    "Game {}: {} / {} ({})",
                    index + 1,
                    player("White"),
                    player("Black"),
                    game.result
                )
            })
            .collect()
    }
    pub(super) fn pgn_entries(&self) -> Vec<String> {
        match self.screen() {
            Screen::PgnImport => self.pgn.files.clone(),
            Screen::PgnGames => self.pgn_game_labels(),
            _ => Vec::new(),
        }
    }
    pub(super) fn pgn_location_text(&self) -> String {
        match self
            .pgn_root()
            .and_then(|root| std::path::absolute(root.join("exports")).map_err(|e| e.to_string()))
        {
            Ok(path) => format!("Drop .pgn files in:\n{}", path.display()),
            Err(error) => format!("PGN error: {error}"),
        }
    }
    fn pgn_result<T>(&mut self, result: Result<T, String>) -> Result<T, String> {
        if let Err(error) = &result {
            self.pgn.message = Some(format!("PGN error: {error}"));
        }
        result
    }
    fn open_pgn_import(&mut self) -> Result<(), String> {
        self.pgn.games.clear();
        self.pgn.files.clear();
        let result = self.pgn_root().and_then(pgn_files::list);
        if let Ok(files) = self.pgn_result(result) {
            self.pgn.files = files;
            self.pgn.message = None;
        }
        self.transition(Screen::PgnImport)
    }
    pub fn import_pgn_file(&mut self, name: &str) -> Result<(), String> {
        let result = self.pgn_root().and_then(|root| pgn_files::read(root, name));
        let games = self.pgn_result(result)?;
        self.pgn.games = games;
        self.pgn.message = None;
        if self.pgn.games.len() == 1 {
            return self.choose_pgn_game(0);
        }
        self.transition(Screen::PgnGames)
    }
    fn choose_pgn_game(&mut self, index: usize) -> Result<(), String> {
        let game = self
            .pgn
            .games
            .get(index)
            .ok_or("PGN game selection is out of range")?;
        let board = self
            .board
            .as_ref()
            .ok_or("editor scenes have no legal game")?;
        let payload = GamePayload::from_game(
            game.state(),
            board.position().seed,
            SavedMode::TwoPlayer,
            imported_result(game.result),
        );
        let result = self.restore_game(payload);
        self.pgn_result(result)?;
        self.pgn.message = Some(format!(
            "Imported game {}. Comments and variations ignored.",
            index + 1
        ));
        self.pgn.games.clear();
        Ok(())
    }
    pub(super) fn pgn_back(&mut self) -> Result<(), String> {
        self.pgn.message = None;
        if self.screen() == Screen::PgnGames {
            self.open_pgn_import()
        } else {
            self.transition(Screen::Slots)
        }
    }
    pub(super) fn pgn_menu_action(&mut self, action: MenuAction) -> Result<(), String> {
        match action {
            MenuAction::ImportPgn => self.open_pgn_import()?,
            MenuAction::ExportPgn => {
                let _ = self.export_pgn();
            }
            MenuAction::PgnEntry(index) if self.screen() == Screen::PgnImport => {
                let name = self
                    .pgn
                    .files
                    .get(index)
                    .ok_or("PGN file selection is out of range")?
                    .clone();
                let _ = self.import_pgn_file(&name);
            }
            MenuAction::PgnEntry(index) => {
                let _ = self.choose_pgn_game(index);
            }
            _ => unreachable!("non-PGN action"),
        }
        self.sync_menu_ui()
    }
    pub fn export_pgn(&mut self) -> Result<std::path::PathBuf, String> {
        let result = self.write_pgn();
        let path = self.pgn_result(result)?;
        self.pgn.message = Some(format!("PGN exported to:\n{}", path.display()));
        Ok(path)
    }
    fn write_pgn(&self) -> Result<std::path::PathBuf, String> {
        let board = self
            .board
            .as_ref()
            .ok_or("editor scenes have no legal game")?;
        let (fen, moves) = board.game().record();
        let initial = ChessState::from_fen(&fen).map_err(|e| e.to_string())?;
        let mut game = PgnGame::new(&initial);
        for uci in moves {
            let chess_move = game.state().parse_uci(&uci).map_err(|e| e.to_string())?;
            game.play(chess_move).map_err(|e| e.to_string())?;
        }
        game.result = export_result(board.game(), self.menus.result.as_deref())?;
        let (date, stamp) = pgn_files::clock()?;
        let player = |side| {
            self.computer_game()
                .filter(|c| c.human() != side)
                .map_or_else(
                    || "Player".to_owned(),
                    |c| {
                        crate::search::LADDER[usize::from(c.level() - 1)]
                            .name
                            .into()
                    },
                )
        };
        for (tag, value) in [
            ("Event", "Casual game".into()),
            ("Site", "rd-chess".into()),
            ("Date", date),
            ("Round", "-".into()),
            ("White", player(Side::Ivory)),
            ("Black", player(Side::Onyx)),
        ] {
            game.tags.insert(tag.into(), value);
        }
        let text = game.export().map_err(|e| e.to_string())?;
        pgn_files::write(self.pgn_root()?, &stamp, &text)
    }
}

fn imported_result(result: PgnResult) -> Option<String> {
    match result {
        PgnResult::Unfinished => None,
        PgnResult::WhiteWin => Some("White wins: imported PGN result".into()),
        PgnResult::BlackWin => Some("Black wins: imported PGN result".into()),
        PgnResult::Draw => Some("Draw: imported PGN result".into()),
    }
}

fn export_result(game: &ChessState, result: Option<&str>) -> Result<PgnResult, String> {
    if let Some(result) = result {
        if result.contains("White wins") {
            return Ok(PgnResult::WhiteWin);
        }
        if result.contains("Black wins") {
            return Ok(PgnResult::BlackWin);
        }
        if result.starts_with("Draw:") {
            return Ok(PgnResult::Draw);
        }
        return Err(format!("unrecognized game result: {result}"));
    }
    Ok(match game.status() {
        GameStatus::Checkmate {
            winner: Side::Ivory,
        } => PgnResult::WhiteWin,
        GameStatus::Checkmate { winner: Side::Onyx } => PgnResult::BlackWin,
        GameStatus::Draw(_) => PgnResult::Draw,
        GameStatus::Ongoing { .. } => PgnResult::Unfinished,
    })
}
