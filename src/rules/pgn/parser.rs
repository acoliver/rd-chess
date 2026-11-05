use super::lexer::{Lexer, Token, tag_pair};
use super::{ChessState, INITIAL_FEN, MAX_INPUT_BYTES, MAX_VARIATION_DEPTH};
use super::{LegalMove, PgnError, PgnErrorKind, PgnGame, PgnResult};
use std::collections::BTreeMap;

fn follows_number(token: Token<'_>) -> bool {
    matches!(token, Token::Comment(_))
        || matches!(token, Token::Word(text) if PgnResult::parse(text).is_none() && !annotation(text))
}

fn annotation(text: &str) -> bool {
    matches!(text, "!" | "?" | "!!" | "??" | "!?" | "?!")
}

/// Import all games atomically. No partial games are returned on a later error.
/// The byte limit is checked before tokenization; variations are legal alternate
/// continuations of the position before the immediately preceding move.
pub fn parse(text: &str) -> Result<Vec<PgnGame>, PgnError> {
    if text.len() > MAX_INPUT_BYTES {
        return Err(PgnError::new(1, 1, "<input>", PgnErrorKind::InputLimit));
    }
    let mut parser = Parser {
        lexer: Lexer::new(text),
        pending: None,
        index: 1,
    };
    let mut games = Vec::new();
    while let Some(mut game) = parser.header()? {
        parser.line(&mut game, 0, 0)?;
        games.push(game);
        parser.index += 1;
    }
    Ok(games)
}

struct Parser<'a> {
    lexer: Lexer<'a>,
    pending: Option<Token<'a>>,
    index: usize,
}

impl<'a> Parser<'a> {
    fn error(&self, ply: usize, token: &str, kind: PgnErrorKind) -> PgnError {
        PgnError::new(self.index, ply, token, kind)
    }

    fn next(&mut self, ply: usize) -> Result<Option<Token<'a>>, PgnError> {
        if let Some(token) = self.pending.take() {
            return Ok(Some(token));
        }
        self.lexer
            .next()
            .map_err(|(token, kind)| self.error(ply, token, kind))
    }

    fn header(&mut self) -> Result<Option<PgnGame>, PgnError> {
        let mut tags = BTreeMap::new();
        let mut comments = Vec::new();
        loop {
            match self.next(1)? {
                Some(Token::Tag(raw)) => {
                    let (name, value) = tag_pair(raw).map_err(|kind| self.error(1, raw, kind))?;
                    if tags.insert(name.clone(), value).is_some() {
                        return Err(self.error(1, &name, PgnErrorKind::Tag));
                    }
                }
                Some(Token::Comment(text)) => comments.push(text),
                token => {
                    if token.is_none() && tags.is_empty() {
                        return Ok(None);
                    }
                    self.pending = token;
                    break;
                }
            }
        }
        let mut game = self.start(tags)?;
        for comment in comments {
            append_comment(&mut game, comment);
        }
        Ok(Some(game))
    }

    fn start(&self, tags: BTreeMap<String, String>) -> Result<PgnGame, PgnError> {
        let fen = match (tags.get("SetUp").map(String::as_str), tags.get("FEN")) {
            (Some("1"), Some(fen)) => fen,
            (None | Some("0"), None) => INITIAL_FEN,
            (_, Some(_)) => return Err(self.error(1, "FEN", PgnErrorKind::Tag)),
            _ => return Err(self.error(1, "SetUp", PgnErrorKind::Tag)),
        };
        if let Some(result) = tags.get("Result")
            && PgnResult::parse(result).is_none()
        {
            return Err(self.error(1, result, PgnErrorKind::Tag));
        }
        let state = ChessState::from_fen(fen)
            .map_err(|error| self.error(1, fen, PgnErrorKind::Fen(error)))?;
        let mut game = PgnGame::new(&state);
        game.tags = tags;
        Ok(game)
    }

    fn line(&mut self, game: &mut PgnGame, depth: usize, base: usize) -> Result<(), PgnError> {
        let mut number_pending = false;
        loop {
            let ply = base + game.moves.len() + 1;
            let Some(token) = self.next(ply)? else {
                let kind = if depth == 0 && !number_pending {
                    PgnErrorKind::MissingResult
                } else {
                    PgnErrorKind::Syntax
                };
                return Err(self.error(ply, "<eof>", kind));
            };
            if number_pending && !follows_number(token) {
                return Err(self.error(ply, token.text(), PgnErrorKind::Syntax));
            }
            match token {
                Token::Comment(text) => {
                    if depth == 0 {
                        append_comment(game, text);
                    }
                }
                Token::Nag(text) => self.nag(text, ply, !game.moves.is_empty())?,
                Token::Word(text) if annotation(text) && !game.moves.is_empty() => (),
                Token::Number(text) => {
                    self.number(text, ply, &game.state)?;
                    number_pending = true;
                }
                Token::Open => self.variation(game, depth, base)?,
                Token::Close if depth > 0 && !game.moves.is_empty() => return Ok(()),
                Token::Word(text) => {
                    if let Some(result) = PgnResult::parse(text) {
                        return self.finish(game, result, text, depth, ply);
                    }
                    self.play(game, text, ply)?;
                    number_pending = false;
                }
                _ => return Err(self.error(ply, token.text(), PgnErrorKind::Syntax)),
            }
        }
    }

    fn variation(&mut self, game: &PgnGame, depth: usize, base: usize) -> Result<(), PgnError> {
        let ply = base + game.moves.len() + 1;
        if depth >= MAX_VARIATION_DEPTH {
            return Err(self.error(ply, "(", PgnErrorKind::NestingLimit));
        }
        if game.moves.is_empty() {
            return Err(self.error(ply, "(", PgnErrorKind::Syntax));
        }
        let mut state = game.state.clone();
        state.undo().expect("variation replaces the preceding move");
        let mut branch = PgnGame {
            initial_fen: state.to_fen(),
            state,
            tags: BTreeMap::new(),
            result: PgnResult::Unfinished,
            moves: Vec::new(),
            comments: BTreeMap::new(),
        };
        self.line(&mut branch, depth + 1, ply - 2)
    }

    fn play(&self, game: &mut PgnGame, text: &str, ply: usize) -> Result<(), PgnError> {
        let chess_move = import_san(&game.state, text)
            .ok_or_else(|| self.error(ply, text, PgnErrorKind::Move(super::MoveError::Notation)))?;
        game.state
            .play(chess_move)
            .map_err(|error| self.error(ply, text, PgnErrorKind::Move(error)))?;
        game.moves.push(chess_move);
        Ok(())
    }

    fn number(&self, text: &str, ply: usize, state: &ChessState) -> Result<(), PgnError> {
        let number = text.trim_end_matches('.');
        let dots = text.len() - number.len();
        let correct_side = dots == 1 && state.side_to_move() == crate::Side::Ivory
            || dots == 3 && state.side_to_move() == crate::Side::Onyx;
        if number.parse::<u32>().ok() != Some(state.fullmove_number()) || !correct_side {
            return Err(self.error(ply, text, PgnErrorKind::Syntax));
        }
        Ok(())
    }

    fn nag(&self, text: &str, ply: usize, follows_move: bool) -> Result<(), PgnError> {
        let digits = &text[1..];
        if !follows_move
            || digits.is_empty()
            || !digits.bytes().all(|b| b.is_ascii_digit())
            || digits.parse::<u8>().is_err()
        {
            return Err(self.error(ply, text, PgnErrorKind::Syntax));
        }
        Ok(())
    }

    fn finish(
        &mut self,
        game: &mut PgnGame,
        result: PgnResult,
        text: &str,
        depth: usize,
        ply: usize,
    ) -> Result<(), PgnError> {
        if depth > 0 {
            while let Some(token) = self.next(ply)? {
                match token {
                    Token::Comment(_) => (),
                    Token::Close if !game.moves.is_empty() => return Ok(()),
                    _ => return Err(self.error(ply, token.text(), PgnErrorKind::Syntax)),
                }
            }
            return Err(self.error(ply, "<eof>", PgnErrorKind::Syntax));
        }
        if game
            .tags
            .get("Result")
            .is_some_and(|tag| PgnResult::parse(tag) != Some(result))
        {
            return Err(self.error(ply, text, PgnErrorKind::ResultMismatch));
        }
        game.result = result;
        Ok(())
    }
}

fn append_comment(game: &mut PgnGame, text: &str) {
    let comment = game.comments.entry(game.moves.len()).or_default();
    let normalized = super::normalize_comment(text);
    if !comment.is_empty() && !normalized.is_empty() {
        comment.push(' ');
    }
    comment.push_str(&normalized);
}

fn import_san(state: &ChessState, text: &str) -> Option<LegalMove> {
    let body = text.trim_end_matches(['!', '?']);
    if body.len() != text.len() && !annotation(&text[body.len()..]) {
        return None;
    }
    let text = body;
    let mut normalized = text.to_owned();
    if normalized.starts_with("0-0") {
        normalized = normalized.replace('0', "O");
    }
    let suffix = normalized.trim_end_matches(['+', '#']).len();
    if suffix >= 2 {
        let bytes = normalized.as_bytes();
        if b"QRBN".contains(&bytes[suffix - 1]) && matches!(bytes[suffix - 2], b'1' | b'8') {
            normalized.insert(suffix - 1, '=');
        }
    }
    state.parse_san(&normalized).ok().or_else(|| {
        if normalized.ends_with(['+', '#']) {
            return None;
        }
        state
            .parse_san(&format!("{normalized}+"))
            .ok()
            .or_else(|| state.parse_san(&format!("{normalized}#")).ok())
    })
}
