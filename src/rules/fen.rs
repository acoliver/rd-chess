use super::{CastlingRights, ChessState, Core, Man, PieceKind, Side};
use std::fmt::{self, Write};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum FenError {
    #[error("FEN must contain exactly six fields")]
    Fields,
    #[error("invalid FEN board placement")]
    Board,
    #[error("invalid side to move")]
    Side,
    #[error("invalid or inconsistent castling rights")]
    Castling,
    #[error("invalid or inconsistent en-passant target")]
    EnPassant,
    #[error("invalid move counters")]
    Counters,
    #[error("exactly one king per side is required")]
    Kings,
    #[error("kings cannot occupy adjacent squares")]
    AdjacentKings,
    #[error("pawns cannot occupy the first or eighth ranks")]
    PawnRank,
    #[error("side not to move is in check")]
    PreviousSideInCheck,
    #[error("piece counts exceed the available material and promotion budget")]
    Material,
}

impl ChessState {
    /// Strict six-field FEN import. A double push may set EP without a capturer,
    /// as required by FEN; repetition hashing includes EP only when legally usable.
    pub fn from_fen(fen: &str) -> Result<Self, FenError> {
        let fields: Vec<_> = fen.split_whitespace().collect();
        let [placement, side, castling, ep, halfmove, fullmove] = fields.as_slice() else {
            return Err(FenError::Fields);
        };
        let core = Core {
            board: parse_board(placement)?,
            kings: [0; 2],
            side: match *side {
                "w" => Side::Ivory,
                "b" => Side::Onyx,
                _ => return Err(FenError::Side),
            },
            rights: parse_rights(castling)?,
            ep: if *ep == "-" {
                None
            } else {
                Some(parse_square(ep).ok_or(FenError::EnPassant)?)
            },
        };
        let mut state = Self {
            core,
            halfmove: parse_counter(halfmove)?,
            fullmove: parse_counter(fullmove)?,
            repetition: Vec::new(),
            undo: Vec::new(),
        };
        if state.fullmove == 0 {
            return Err(FenError::Counters);
        }
        state.validate()?;
        state.repetition.push(state.core.zobrist());
        Ok(state)
    }

    pub fn to_fen(&self) -> String {
        let ep = self.core.ep.map_or_else(|| "-".into(), square_name);
        format!(
            "{} {} {} {} {} {}",
            self.core.placement(),
            if self.core.side == Side::Ivory {
                "w"
            } else {
                "b"
            },
            self.core.rights,
            ep,
            self.halfmove,
            self.fullmove
        )
    }
}

fn parse_counter(field: &str) -> Result<u32, FenError> {
    if field.is_empty() || !field.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(FenError::Counters);
    }
    field.parse().map_err(|_| FenError::Counters)
}

fn parse_board(placement: &str) -> Result<[Option<Man>; 128], FenError> {
    let ranks: Vec<_> = placement.split('/').collect();
    if ranks.len() != 8 {
        return Err(FenError::Board);
    }
    let mut board = [None; 128];
    for (row, text) in ranks.iter().enumerate() {
        let rank = 7 - u8::try_from(row).expect("eight ranks");
        parse_rank(text, rank, &mut board)?;
    }
    Ok(board)
}

fn parse_rank(text: &str, rank: u8, board: &mut [Option<Man>; 128]) -> Result<(), FenError> {
    let mut file = 0_u8;
    let mut previous_digit = false;
    for byte in text.bytes() {
        if (b'1'..=b'8').contains(&byte) {
            if previous_digit {
                return Err(FenError::Board);
            }
            file = file.checked_add(byte - b'0').ok_or(FenError::Board)?;
            previous_digit = true;
        } else {
            if file >= 8 {
                return Err(FenError::Board);
            }
            let location = rank * 16 + file;
            board[usize::from(location)] = Some(Man {
                side: if byte.is_ascii_uppercase() {
                    Side::Ivory
                } else {
                    Side::Onyx
                },
                kind: parse_kind(byte).ok_or(FenError::Board)?,
                id: location,
            });
            file += 1;
            previous_digit = false;
        }
        if file > 8 {
            return Err(FenError::Board);
        }
    }
    if file == 8 {
        Ok(())
    } else {
        Err(FenError::Board)
    }
}

fn parse_rights(text: &str) -> Result<CastlingRights, FenError> {
    if text == "-" {
        return Ok(CastlingRights(0));
    }
    if text.is_empty() {
        return Err(FenError::Castling);
    }
    let mut mask = 0;
    for byte in text.bytes() {
        let bit = match byte {
            b'K' => 1,
            b'Q' => 2,
            b'k' => 4,
            b'q' => 8,
            _ => return Err(FenError::Castling),
        };
        if mask & bit != 0 {
            return Err(FenError::Castling);
        }
        mask |= bit;
    }
    Ok(CastlingRights(mask))
}

pub(super) fn parse_kind(byte: u8) -> Option<PieceKind> {
    match byte.to_ascii_lowercase() {
        b'p' => Some(PieceKind::Pawn),
        b'n' => Some(PieceKind::Knight),
        b'b' => Some(PieceKind::Bishop),
        b'r' => Some(PieceKind::Rook),
        b'q' => Some(PieceKind::Queen),
        b'k' => Some(PieceKind::King),
        _ => None,
    }
}

pub(super) fn kind_letter(kind: PieceKind) -> char {
    match kind {
        PieceKind::Pawn => 'P',
        PieceKind::Knight => 'N',
        PieceKind::Bishop => 'B',
        PieceKind::Rook => 'R',
        PieceKind::Queen => 'Q',
        PieceKind::King => 'K',
    }
}

pub(super) fn parse_square(text: &str) -> Option<u8> {
    let [file, rank] = *text.as_bytes() else {
        return None;
    };
    if !(b'a'..=b'h').contains(&file) || !(b'1'..=b'8').contains(&rank) {
        return None;
    }
    Some((rank - b'1') * 16 + file - b'a')
}

pub(super) fn square_name(location: u8) -> String {
    let location = super::square(location);
    format!("{}{}", char::from(b'a' + location.file), location.rank + 1)
}

impl fmt::Display for CastlingRights {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            return formatter.write_str("-");
        }
        for (bit, letter) in [(1, 'K'), (2, 'Q'), (4, 'k'), (8, 'q')] {
            if self.0 & bit != 0 {
                formatter.write_char(letter)?;
            }
        }
        Ok(())
    }
}

impl Core {
    fn placement(&self) -> String {
        let mut result = String::new();
        for rank in (0_u8..8).rev() {
            let mut empty = 0_u8;
            for file in 0..8 {
                if let Some(man) = self.at(rank * 16 + file) {
                    flush_empty(&mut result, &mut empty);
                    let letter = kind_letter(man.kind);
                    result.push(if man.side == Side::Ivory {
                        letter
                    } else {
                        letter.to_ascii_lowercase()
                    });
                } else {
                    empty += 1;
                }
            }
            flush_empty(&mut result, &mut empty);
            if rank != 0 {
                result.push('/');
            }
        }
        result
    }
}

fn flush_empty(text: &mut String, empty: &mut u8) {
    if *empty > 0 {
        text.push(char::from(b'0' + *empty));
        *empty = 0;
    }
}
