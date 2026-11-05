use super::PgnErrorKind;

#[derive(Clone, Copy, Debug)]
pub(super) enum Token<'a> {
    Tag(&'a str),
    Comment(&'a str),
    Number(&'a str),
    Word(&'a str),
    Nag(&'a str),
    Open,
    Close,
}

impl<'a> Token<'a> {
    pub fn text(self) -> &'a str {
        match self {
            Self::Tag(s) | Self::Comment(s) | Self::Number(s) | Self::Word(s) | Self::Nag(s) => s,
            Self::Open => "(",
            Self::Close => ")",
        }
    }
}

pub(super) struct Lexer<'a> {
    rest: &'a str,
}

type LexResult<'a> = Result<Option<Token<'a>>, (&'a str, PgnErrorKind)>;

impl<'a> Lexer<'a> {
    pub fn new(text: &'a str) -> Self {
        Self { rest: text }
    }

    pub fn next(&mut self) -> LexResult<'a> {
        self.rest = self.rest.trim_start();
        let Some(first) = self.rest.chars().next() else {
            return Ok(None);
        };
        match first {
            '[' => self.tag(),
            '{' => self.brace_comment(),
            ';' => {
                let length = self.rest.find('\n').unwrap_or(self.rest.len());
                let raw = self.take(length);
                if super::valid_comment(&raw[1..]) {
                    Ok(Some(Token::Comment(&raw[1..])))
                } else {
                    Err((raw, PgnErrorKind::Syntax))
                }
            }
            '(' | ')' => {
                self.take(1);
                Ok(Some(if first == '(' {
                    Token::Open
                } else {
                    Token::Close
                }))
            }
            '}' | ']' | '"' => Err((self.take(first.len_utf8()), PgnErrorKind::Syntax)),
            _ => Ok(Some(self.word())),
        }
    }

    fn take(&mut self, length: usize) -> &'a str {
        let (text, rest) = self.rest.split_at(length);
        self.rest = rest;
        text
    }

    fn tag(&mut self) -> LexResult<'a> {
        let mut quoted = false;
        let mut escaped = false;
        for (index, c) in self.rest.char_indices().skip(1) {
            if escaped {
                escaped = false;
            } else if c == '\\' && quoted {
                escaped = true;
            } else if c == '"' {
                quoted = !quoted;
            } else if c == ']' && !quoted {
                return Ok(Some(Token::Tag(self.take(index + 1))));
            }
        }
        Err((self.take(self.rest.len()), PgnErrorKind::Tag))
    }

    fn brace_comment(&mut self) -> LexResult<'a> {
        let Some(end) = self.rest.find('}') else {
            return Err((self.take(self.rest.len()), PgnErrorKind::Syntax));
        };
        let text = self.take(end + 1);
        let body = &text[1..text.len() - 1];
        if !body.contains('{') && super::valid_comment(body) {
            Ok(Some(Token::Comment(body)))
        } else {
            Err((text, PgnErrorKind::Syntax))
        }
    }

    fn word(&mut self) -> Token<'a> {
        let digits = self.rest.bytes().take_while(u8::is_ascii_digit).count();
        if digits > 0 && self.rest.as_bytes().get(digits) == Some(&b'.') {
            let dots = self.rest[digits..]
                .bytes()
                .take_while(|&b| b == b'.')
                .count();
            return Token::Number(self.take(digits + dots));
        }
        let length = self
            .rest
            .char_indices()
            .skip(1)
            .find(|(_, c)| delimiter(*c))
            .map_or(self.rest.len(), |(index, _)| index);
        let text = self.take(length);
        if text.starts_with('$') {
            Token::Nag(text)
        } else {
            Token::Word(text)
        }
    }
}

fn delimiter(c: char) -> bool {
    c.is_whitespace() || "[]{}();$\"".contains(c)
}

pub(super) fn tag_pair(raw: &str) -> Result<(String, String), PgnErrorKind> {
    let inner = raw[1..raw.len() - 1].trim();
    let split = inner.find(char::is_whitespace).ok_or(PgnErrorKind::Tag)?;
    let name = &inner[..split];
    let quoted = inner[split..].trim();
    if !super::valid_tag_name(name)
        || !quoted.starts_with('"')
        || !quoted.ends_with('"')
        || quoted.len() < 2
    {
        return Err(PgnErrorKind::Tag);
    }
    let mut value = String::new();
    let mut chars = quoted[1..quoted.len() - 1].chars();
    while let Some(c) = chars.next() {
        match c {
            '\\' => match chars.next() {
                Some(escaped @ ('\\' | '"')) => value.push(escaped),
                _ => return Err(PgnErrorKind::Tag),
            },
            '"' => return Err(PgnErrorKind::Tag),
            c if c.is_control() => return Err(PgnErrorKind::Tag),
            c => value.push(c),
        }
    }
    Ok((name.into(), value))
}
