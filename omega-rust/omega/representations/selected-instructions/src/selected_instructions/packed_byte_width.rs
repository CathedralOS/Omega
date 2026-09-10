//! Exact non-power-of-two byte footprints, without widening memory access.

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PackedByteWidth {
    Three,
    Five,
    Six,
    Seven,
}

impl PackedByteWidth {
    pub const fn byte_size(self) -> u8 {
        match self {
            Self::Three => 3,
            Self::Five => 5,
            Self::Six => 6,
            Self::Seven => 7,
        }
    }

    pub const fn from_byte_size(bytes: u8) -> Option<Self> {
        match bytes {
            3 => Some(Self::Three),
            5 => Some(Self::Five),
            6 => Some(Self::Six),
            7 => Some(Self::Seven),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn packed_widths_admit_only_exact_odd_fragments() {
        for raw in 0..=u8::MAX {
            let decoded = PackedByteWidth::from_byte_size(raw);
            assert_eq!(decoded.is_some(), matches!(raw, 3 | 5 | 6 | 7));
            if let Some(width) = decoded {
                assert_eq!(width.byte_size(), raw);
            }
        }
    }
}
