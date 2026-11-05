use super::board::{KING_STEPS, KNIGHT_STEPS};

#[derive(Clone, Copy)]
pub(super) struct Targets {
    squares: [u8; 8],
    len: u8,
}

impl Targets {
    pub fn iter(&self) -> impl Iterator<Item = u8> + '_ {
        self.squares[..usize::from(self.len)].iter().copied()
    }
}

pub(super) const KNIGHTS: [Targets; 128] = targets(&KNIGHT_STEPS);
pub(super) const KINGS: [Targets; 128] = targets(&KING_STEPS);
pub(super) const PAWNS: [[Targets; 128]; 2] = [targets(&[15, 17]), targets(&[-17, -15])];

const fn targets(steps: &[i16]) -> [Targets; 128] {
    let mut result = [Targets {
        squares: [0; 8],
        len: 0,
    }; 128];
    let mut from = 0_u8;
    while from < 128 {
        if from & 0x88 == 0 {
            let mut step = 0;
            while step < steps.len() {
                let to = from as i16 + steps[step];
                if to >= 0 && to < 128 && to & 0x88 == 0 {
                    let entry = &mut result[from as usize];
                    entry.squares[entry.len as usize] = to as u8;
                    entry.len += 1;
                }
                step += 1;
            }
        }
        from += 1;
    }
    result
}
