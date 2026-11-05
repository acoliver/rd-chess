use super::{ChessState, INITIAL_FEN, PgnError, PgnErrorKind, PgnGame, valid_tag_name};
use std::fmt::Write;

const ROSTER: [(&str, &str); 7] = [
    ("Event", "Chess3D"),
    ("Site", "?"),
    ("Date", "????.??.??"),
    ("Round", "-"),
    ("White", "White"),
    ("Black", "Black"),
    ("Result", "*"),
];

impl PgnGame {
    /// Emit export-format roster order, standard SAN and lines of at most 80
    /// columns in movetext. A comment word longer than 78 columns is rejected
    /// rather than silently changing its contents to introduce a line break.
    pub fn export(&self) -> Result<String, PgnError> {
        let mut output = self.header()?;
        let mut state = ChessState::from_fen(&self.initial_fen).expect("validated initial FEN");
        let mut text = Movetext::default();
        text.comment(self.comment(0), 0)?;
        for (index, &chess_move) in self.moves.iter().enumerate() {
            if state.side_to_move() == crate::Side::Ivory {
                text.word(&format!("{}.", state.fullmove_number()));
            } else if index == 0 || self.comment(index).is_some() {
                text.word(&format!("{}...", state.fullmove_number()));
            }
            let san = state.to_san(chess_move).expect("recorded move is legal");
            text.word(&san);
            state.play(chess_move).expect("recorded move counters fit");
            text.comment(self.comment(index + 1), index + 1)?;
        }
        text.word(&self.result.to_string());
        output.push_str(&text.output);
        output.push('\n');
        Ok(output)
    }

    fn header(&self) -> Result<String, PgnError> {
        for (name, value) in &self.tags {
            if !valid_tag_name(name) || value.chars().any(char::is_control) {
                return Err(PgnError::new(1, 1, name, PgnErrorKind::Tag));
            }
        }
        let mut tags = self.tags.clone();
        tags.insert("Result".into(), self.result.to_string());
        tags.remove("SetUp");
        tags.remove("FEN");
        if self.initial_fen != INITIAL_FEN {
            tags.insert("SetUp".into(), "1".into());
            tags.insert("FEN".into(), self.initial_fen.clone());
        }
        let mut output = String::new();
        for (name, default) in ROSTER {
            tag(
                &mut output,
                name,
                &tags.remove(name).unwrap_or_else(|| default.into()),
            );
        }
        for (name, value) in tags {
            tag(&mut output, &name, &value);
        }
        output.push('\n');
        Ok(output)
    }
}

fn tag(output: &mut String, name: &str, value: &str) {
    let escaped = value.replace('\\', "\\\\").replace('"', "\\\"");
    writeln!(output, "[{name} \"{escaped}\"]").expect("writing to String");
}

#[derive(Default)]
struct Movetext {
    output: String,
    column: usize,
}

impl Movetext {
    fn word(&mut self, word: &str) {
        let length = word.chars().count();
        if self.column > 0 {
            if self.column + 1 + length > 80 {
                self.output.push('\n');
                self.column = 0;
            } else {
                self.output.push(' ');
                self.column += 1;
            }
        }
        self.output.push_str(word);
        self.column += length;
    }

    fn comment(&mut self, comment: Option<&str>, ply: usize) -> Result<(), PgnError> {
        let Some(comment) = comment else {
            return Ok(());
        };
        if comment
            .split_whitespace()
            .any(|word| word.chars().count() > 78)
        {
            return Err(PgnError::new(1, ply, comment, PgnErrorKind::Comment));
        }
        if comment.contains(['{', '}']) {
            self.line_comment(comment);
        } else {
            self.word("{");
            for word in comment.split_whitespace() {
                self.word(word);
            }
            self.word("}");
        }
        Ok(())
    }

    fn newline(&mut self) {
        self.output.push('\n');
        self.column = 0;
    }

    fn line_comment(&mut self, comment: &str) {
        if self.column > 0 {
            self.newline();
        }
        self.word(";");
        for word in comment.split_whitespace() {
            if self.column + 1 + word.chars().count() > 80 {
                self.newline();
                self.word(";");
            }
            self.word(word);
        }
        self.newline();
    }
}
