use super::{Core, Man, PieceKind, Side};

// Stateless SplitMix64 supplies fixed independent-looking Zobrist keys without
// global initialization. Domains below do not overlap. Identity excludes IDs,
// counters and unusable EP targets, which cannot change legal move availability.
pub(super) fn key(ordinal: u64) -> u64 {
    let mut value = ordinal.wrapping_add(0x8a5c_d789_635d_2dff);
    value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
    value ^ (value >> 31)
}

pub(super) fn piece_key(location: u8, man: Option<Man>) -> u64 {
    let Some(man) = man else {
        return 0;
    };
    let kind = match man.kind {
        PieceKind::Pawn => 0,
        PieceKind::Knight => 1,
        PieceKind::Bishop => 2,
        PieceKind::Rook => 3,
        PieceKind::Queen => 4,
        PieceKind::King => 5,
    };
    let colour = u64::from(man.side == Side::Onyx) * 6;
    key(u64::from(location) * 12 + colour + kind)
}

impl Core {
    pub fn ep_hash(&self) -> u64 {
        if self.legal_ep_available() {
            key(1541 + u64::from(self.ep.expect("usable ep exists") & 7))
        } else {
            0
        }
    }

    pub fn zobrist(&self) -> u64 {
        let mut hash = self.ep_hash();
        for (location, man) in self.men() {
            hash ^= piece_key(location, Some(man));
        }
        if self.side == Side::Onyx {
            hash ^= key(1536);
        }
        for bit in 0..4 {
            if self.rights.0 & (1 << bit) != 0 {
                hash ^= key(1537 + bit);
            }
        }
        hash
    }
}
