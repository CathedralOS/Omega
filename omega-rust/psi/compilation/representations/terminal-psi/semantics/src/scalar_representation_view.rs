//! The closed pairing and bit meaning of `ScalarRepresentationView`.
//!
//! A representation re-view reads one scalar's stored bits at another
//! carrier of the same width ([recasts](../../../../../../wiki/spec/layouts/recasts.md)).
//! Equal-width integer pairs already have a Terminal spelling, the modular
//! image the integer operations compose, so this operation owns only the
//! cross-family pairs that no other operation spells:
//!
//! - an IEEE binary32 (binary64) value and a 32-bit (64-bit) integer of
//!   either sign, in either direction: the integer is the interchange
//!   encoding, two's complement for a signed carrier, so every NaN payload
//!   and both zeros survive;
//! - a Boolean and an 8-bit integer of either sign: a Boolean is stored as
//!   the byte `0` or `1`, so the Boolean-to-byte direction is total and the
//!   byte-to-Boolean direction exists only for a byte already in `{0, 1}`.
//!
//! Width is the carrier's storage width, never a numeric range: equal byte
//! counts do not admit Boolean against a float, a float against the other
//! format, or two floats. The validator, the obligation reconstruction, the
//! Terminal interpreter and Omega's lowering all read this one table, so a
//! pair admitted anywhere is admitted everywhere.

use semantic_vocabulary::{
    IeeeFloatFormat, IeeeFloatValue, IntegerSign, IntegerType, IntegerValue, Proposition,
    ScalarTerm, ScalarType, ValueId,
};

/// One admitted `(source, target)` carrier pair of a representation re-view.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarRepresentationViewPair {
    /// IEEE interchange bits read at an integer carrier of the same width.
    FloatBits {
        format: IeeeFloatFormat,
        integer: IntegerType,
    },
    /// An integer's stored bits read as an IEEE value of the same width.
    BitsFloat {
        integer: IntegerType,
        format: IeeeFloatFormat,
    },
    /// A Boolean's `{0, 1}` byte read at an 8-bit integer carrier.
    BooleanByte { integer: IntegerType },
    /// An 8-bit integer already proven in `{0, 1}` read as a Boolean.
    ByteBoolean { integer: IntegerType },
}

impl ScalarRepresentationViewPair {
    /// Classify one carrier pair, or `None` when the pair has unequal widths,
    /// does not change family, or reaches a non-fixed integer carrier.
    pub fn classify(source: ScalarType, target: ScalarType) -> Option<Self> {
        let fixed =
            |integer: IntegerType, bits: u16| !integer.is_address() && integer.bits() == bits;
        match (source, target) {
            (ScalarType::IeeeFloat(format), ScalarType::Integer(integer))
                if fixed(integer, format_bits(format)) =>
            {
                Some(Self::FloatBits { format, integer })
            }
            (ScalarType::Integer(integer), ScalarType::IeeeFloat(format))
                if fixed(integer, format_bits(format)) =>
            {
                Some(Self::BitsFloat { integer, format })
            }
            (ScalarType::Boolean, ScalarType::Integer(integer)) if fixed(integer, 8) => {
                Some(Self::BooleanByte { integer })
            }
            (ScalarType::Integer(integer), ScalarType::Boolean) if fixed(integer, 8) => {
                Some(Self::ByteBoolean { integer })
            }
            _ => None,
        }
    }

    /// Whether the operand must first be proven in `{0, 1}`.
    pub const fn requires_landing(self) -> bool {
        matches!(self, Self::ByteBoolean { .. })
    }

    /// The canonical landing goal `0 <= operand && operand <= 1` at the
    /// operand's own carrier, or `None` for a total pair. This is the same
    /// closed-interval conjunction a bounded field store owes, so the same
    /// interval facts (a `[0..=1]` field, a data `where` bound, a guard)
    /// discharge it.
    pub fn landing_goal(self, operand: ValueId) -> Option<Proposition> {
        let Self::ByteBoolean { integer } = self else {
            return None;
        };
        let endpoint = |value: u8| ScalarTerm::Integer {
            scalar_type: integer,
            value: match integer.sign() {
                IntegerSign::Signed => IntegerValue::Signed(i128::from(value)),
                IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(value)),
            },
        };
        let operand = ScalarTerm::value(operand, ScalarType::Integer(integer));
        let mut clauses = vec![
            Proposition::LessOrEqual(endpoint(0), operand.clone()),
            Proposition::LessOrEqual(operand, endpoint(1)),
        ];
        clauses.sort();
        Some(Proposition::Conjunction(clauses))
    }

    /// Re-view one source value at the target carrier, bit for bit. `None`
    /// when the value does not belong to the source carrier, or when a byte
    /// landing on `bool` is outside `{0, 1}` (the landing goal excludes it).
    pub fn view(self, value: ScalarViewValue) -> Option<ScalarViewValue> {
        match (self, value) {
            (Self::FloatBits { format, integer }, ScalarViewValue::IeeeFloat(float))
                if float.format() == format =>
            {
                let bits = match float {
                    IeeeFloatValue::Binary32(bits) => u64::from(bits),
                    IeeeFloatValue::Binary64(bits) => bits,
                };
                Some(ScalarViewValue::Integer(integer_from_bits(integer, bits)))
            }
            (Self::BitsFloat { integer, format }, ScalarViewValue::Integer(value)) => {
                let bits = integer_bits(integer, value)?;
                Some(ScalarViewValue::IeeeFloat(match format {
                    IeeeFloatFormat::Binary32 => {
                        IeeeFloatValue::Binary32(u32::try_from(bits).ok()?)
                    }
                    IeeeFloatFormat::Binary64 => IeeeFloatValue::Binary64(bits),
                }))
            }
            (Self::BooleanByte { integer }, ScalarViewValue::Boolean(value)) => Some(
                ScalarViewValue::Integer(integer_from_bits(integer, u64::from(value))),
            ),
            (Self::ByteBoolean { integer }, ScalarViewValue::Integer(value)) => {
                match integer_bits(integer, value)? {
                    0 => Some(ScalarViewValue::Boolean(false)),
                    1 => Some(ScalarViewValue::Boolean(true)),
                    _ => None,
                }
            }
            _ => None,
        }
    }
}

/// One scalar value as a re-view reads or produces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarViewValue {
    Boolean(bool),
    Integer(IntegerValue),
    IeeeFloat(IeeeFloatValue),
}

const fn format_bits(format: IeeeFloatFormat) -> u16 {
    match format {
        IeeeFloatFormat::Binary32 => 32,
        IeeeFloatFormat::Binary64 => 64,
    }
}

/// The stored two's-complement bits of `value` at `integer`'s width, or
/// `None` when the value is outside the carrier.
pub(crate) fn integer_bits(integer: IntegerType, value: IntegerValue) -> Option<u64> {
    if !integer.admits(value) || integer.bits() > 64 {
        return None;
    }
    let mask = if integer.bits() == 64 {
        u64::MAX
    } else {
        (1u64 << integer.bits()) - 1
    };
    Some(match value {
        IntegerValue::Signed(value) => (value as u64) & mask,
        IntegerValue::Unsigned(value) => u64::try_from(value).ok()? & mask,
    })
}

/// The value whose stored bits at `integer`'s width are the low bits of
/// `bits`, signed carriers reading them as two's complement.
pub(crate) fn integer_from_bits(integer: IntegerType, bits: u64) -> IntegerValue {
    let width = u32::from(integer.bits());
    match integer.sign() {
        IntegerSign::Unsigned => {
            let mask = if width == 64 {
                u64::MAX
            } else {
                (1u64 << width) - 1
            };
            IntegerValue::Unsigned(u128::from(bits & mask))
        }
        IntegerSign::Signed => {
            let shift = 64 - width;
            IntegerValue::Signed(i128::from(((bits << shift) as i64) >> shift))
        }
    }
}
