//! The base an integer literal's digits are written in.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IntegerRadix {
    Binary,
    Octal,
    Decimal,
    Hexadecimal,
}

impl IntegerRadix {
    pub fn get_base(self) -> u32 {
        match self {
            Self::Binary => 2,
            Self::Octal => 8,
            Self::Decimal => 10,
            Self::Hexadecimal => 16,
        }
    }

    /// How many bytes the radix prefix (`0x`, `0b`, `0o`) takes. Decimal has none.
    pub fn get_prefix_byte_count(self) -> usize {
        match self {
            Self::Decimal => 0,
            Self::Binary | Self::Octal | Self::Hexadecimal => 2,
        }
    }
}
