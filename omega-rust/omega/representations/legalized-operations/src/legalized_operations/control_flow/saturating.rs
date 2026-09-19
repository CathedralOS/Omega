//! The fixed integer carriers whose saturating arithmetic the physical route
//! realizes, shared by the legalized and selected instruction vocabularies.
//!
//! Every carrier is a fixed 8/16/32/64-bit signed or unsigned integer. The
//! scalar transport keeps narrow carriers sign- or zero-normalized in 64-bit
//! registers, which is why one selected kind per operation can serve every
//! width: the realization computes the exact 64-bit result of the normalized
//! operands (or detects the 64-bit overflow flag for the full-width carriers)
//! and clamps to the bounds this carrier names. Address-carrier and other
//! widths are not saturating carriers and stay unsupported at legalization.
use semantic_vocabulary::{IntegerCarrier, IntegerSign, IntegerType};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SaturatingCarrier {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
}

/// The saturating operations the carriers are realized for. Remainder is a
/// member even though the mathematical remainder never exceeds its carrier:
/// the operation still carries the nonzero-divisor obligation, and on signed
/// carriers the one wrapped quotient case (MIN % -1 = 0) is exactly the
/// remainder the signed form produces.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SaturatingOperation {
    Add,
    Subtract,
    Divide,
    Remainder,
}

impl SaturatingCarrier {
    pub const ALL: [Self; 8] = [
        Self::I8,
        Self::I16,
        Self::I32,
        Self::I64,
        Self::U8,
        Self::U16,
        Self::U32,
        Self::U64,
    ];

    /// The carrier of a declared integer type, or `None` for every type
    /// saturating arithmetic is not realized for.
    pub fn from_integer(integer: IntegerType) -> Option<Self> {
        if integer.carrier() != IntegerCarrier::Fixed {
            return None;
        }
        Some(match (integer.sign(), integer.bits()) {
            (IntegerSign::Signed, 8) => Self::I8,
            (IntegerSign::Signed, 16) => Self::I16,
            (IntegerSign::Signed, 32) => Self::I32,
            (IntegerSign::Signed, 64) => Self::I64,
            (IntegerSign::Unsigned, 8) => Self::U8,
            (IntegerSign::Unsigned, 16) => Self::U16,
            (IntegerSign::Unsigned, 32) => Self::U32,
            (IntegerSign::Unsigned, 64) => Self::U64,
            _ => return None,
        })
    }

    /// The declared integer type this carrier stands for.
    pub fn integer_type(self) -> IntegerType {
        IntegerType::new(self.sign(), self.bits()).expect("saturating carriers are fixed widths")
    }

    pub const fn sign(self) -> IntegerSign {
        if self.is_signed() {
            IntegerSign::Signed
        } else {
            IntegerSign::Unsigned
        }
    }

    pub const fn is_signed(self) -> bool {
        matches!(self, Self::I8 | Self::I16 | Self::I32 | Self::I64)
    }

    pub const fn bits(self) -> u16 {
        match self {
            Self::I8 | Self::U8 => 8,
            Self::I16 | Self::U16 => 16,
            Self::I32 | Self::U32 => 32,
            Self::I64 | Self::U64 => 64,
        }
    }

    /// A narrow carrier occupies fewer than 64 bits, so its normalized
    /// operands combine exactly in a 64-bit register and only the final clamp
    /// depends on the carrier; the 64-bit carriers need overflow detection.
    pub const fn is_narrow(self) -> bool {
        self.bits() < 64
    }

    /// The carrier minimum as a 64-bit two's-complement bit pattern.
    pub const fn minimum_bits(self) -> u64 {
        match self {
            Self::I8 => i8::MIN as i64 as u64,
            Self::I16 => i16::MIN as i64 as u64,
            Self::I32 => i32::MIN as i64 as u64,
            Self::I64 => i64::MIN as u64,
            Self::U8 | Self::U16 | Self::U32 | Self::U64 => 0,
        }
    }

    /// The carrier maximum as a 64-bit bit pattern.
    pub const fn maximum_bits(self) -> u64 {
        match self {
            Self::I8 => i8::MAX as u64,
            Self::I16 => i16::MAX as u64,
            Self::I32 => i32::MAX as u64,
            Self::I64 => i64::MAX as u64,
            Self::U8 => u8::MAX as u64,
            Self::U16 => u16::MAX as u64,
            Self::U32 => u32::MAX as u64,
            Self::U64 => u64::MAX,
        }
    }

    /// The dense position of this carrier in `ALL`, which identity tables
    /// add to a per-operation base for the carriers that had no tag before
    /// the family was widened.
    pub const fn ordinal(self) -> u8 {
        match self {
            Self::I8 => 0,
            Self::I16 => 1,
            Self::I32 => 2,
            Self::I64 => 3,
            Self::U8 => 4,
            Self::U16 => 5,
            Self::U32 => 6,
            Self::U64 => 7,
        }
    }
}
