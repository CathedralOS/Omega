//! Integer types and their closed arithmetic: sign, carrier and width, the
//! values and math literals they admit, and the math terms built over them.

use crate::ValueId;
use crate::proposition::PropositionError;
use std::cmp::Ordering;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerSign {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerCarrier {
    Fixed,
    Address,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerType {
    carrier: IntegerCarrier,
    sign: IntegerSign,
    bits: u16,
}

impl IntegerType {
    pub fn can_exact_cast_to(self, target: Self) -> bool {
        self.carrier == IntegerCarrier::Fixed && target.carrier == IntegerCarrier::Fixed
    }

    pub fn exact_cast_value_to(self, target: Self, value: IntegerValue) -> Option<IntegerValue> {
        if !self.can_exact_cast_to(target) || !self.admits(value) {
            return None;
        }
        let converted = match (target.sign, value) {
            (IntegerSign::Signed, IntegerValue::Signed(value)) => IntegerValue::Signed(value),
            (IntegerSign::Signed, IntegerValue::Unsigned(value)) => {
                IntegerValue::Signed(i128::try_from(value).ok()?)
            }
            (IntegerSign::Unsigned, IntegerValue::Signed(value)) => {
                IntegerValue::Unsigned(u128::try_from(value).ok()?)
            }
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => IntegerValue::Unsigned(value),
        };
        target.admits(converted).then_some(converted)
    }

    pub fn minimum_value(self) -> IntegerValue {
        match self.sign {
            IntegerSign::Signed if self.bits == 128 => IntegerValue::Signed(i128::MIN),
            IntegerSign::Signed => IntegerValue::Signed(-(1_i128 << (self.bits - 1))),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        }
    }

    pub fn maximum_value(self) -> IntegerValue {
        match self.sign {
            IntegerSign::Signed if self.bits == 128 => IntegerValue::Signed(i128::MAX),
            IntegerSign::Signed => IntegerValue::Signed((1_i128 << (self.bits - 1)) - 1),
            IntegerSign::Unsigned if self.bits == 128 => IntegerValue::Unsigned(u128::MAX),
            IntegerSign::Unsigned => IntegerValue::Unsigned((1_u128 << self.bits) - 1),
        }
    }

    pub fn can_widen_to(self, target: Self) -> bool {
        self.carrier == IntegerCarrier::Fixed
            && target.carrier == IntegerCarrier::Fixed
            && self.bits < target.bits
            && (self.sign == target.sign
                || matches!(
                    (self.sign, target.sign),
                    (IntegerSign::Unsigned, IntegerSign::Signed)
                ))
    }

    pub fn widen_value_to(self, target: Self, value: IntegerValue) -> Option<IntegerValue> {
        if !self.can_widen_to(target) || !self.admits(value) {
            return None;
        }
        match (target.sign, value) {
            (IntegerSign::Signed, IntegerValue::Unsigned(value)) => {
                Some(IntegerValue::Signed(i128::try_from(value).ok()?))
            }
            (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(IntegerValue::Signed(value)),
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                Some(IntegerValue::Unsigned(value))
            }
            (IntegerSign::Unsigned, IntegerValue::Signed(_)) => None,
        }
    }

    pub fn new(sign: IntegerSign, bits: u16) -> Result<Self, PropositionError> {
        if !(1..=128).contains(&bits) {
            return Err(PropositionError::InvalidIntegerWidth(bits));
        }
        Ok(Self {
            carrier: IntegerCarrier::Fixed,
            sign,
            bits,
        })
    }

    pub fn address(bits: u16) -> Result<Self, PropositionError> {
        if !(1..=128).contains(&bits) {
            return Err(PropositionError::InvalidIntegerWidth(bits));
        }
        Ok(Self {
            carrier: IntegerCarrier::Address,
            sign: IntegerSign::Unsigned,
            bits,
        })
    }

    pub const fn carrier(self) -> IntegerCarrier {
        self.carrier
    }

    pub const fn is_address(self) -> bool {
        matches!(self.carrier, IntegerCarrier::Address)
    }

    pub const fn sign(self) -> IntegerSign {
        self.sign
    }

    pub const fn bits(self) -> u16 {
        self.bits
    }

    pub fn admits(self, value: IntegerValue) -> bool {
        match (self.sign, value) {
            (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                self.bits == 128 || {
                    let limit = 1_i128 << (self.bits - 1);
                    (-limit..limit).contains(&value)
                }
            }
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                self.bits == 128 || value < (1_u128 << self.bits)
            }
            _ => false,
        }
    }

    /// Compare two values under this exact integer type's signedness.
    ///
    /// Values of the wrong sign or outside the declared width reject rather
    /// than being reinterpreted.
    pub fn compare(self, left: IntegerValue, right: IntegerValue) -> Option<Ordering> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(left.cmp(&right)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                Some(left.cmp(&right))
            }
            _ => None,
        }
    }

    pub fn bitwise_and(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.bitwise(
            left,
            right,
            |left, right| left & right,
            |left, right| left & right,
        )
    }

    pub fn bitwise_or(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.bitwise(
            left,
            right,
            |left, right| left | right,
            |left, right| left | right,
        )
    }

    pub fn bitwise_xor(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.bitwise(
            left,
            right,
            |left, right| left ^ right,
            |left, right| left ^ right,
        )
    }

    /// Complement an admitted value within this exact integer width.
    pub fn bitwise_not(self, operand: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(operand) {
            return None;
        }
        match (self.sign, operand) {
            (IntegerSign::Signed, IntegerValue::Signed(operand)) => {
                Some(IntegerValue::Signed(!operand))
            }
            (IntegerSign::Unsigned, IntegerValue::Unsigned(operand)) => {
                let mask = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                Some(IntegerValue::Unsigned(!operand & mask))
            }
            _ => None,
        }
    }

    fn bitwise(
        self,
        left: IntegerValue,
        right: IntegerValue,
        signed: impl FnOnce(i128, i128) -> i128,
        unsigned: impl FnOnce(u128, u128) -> u128,
    ) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(unsigned(left, right))),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                Some(IntegerValue::Signed(signed(left, right)))
            }
            _ => None,
        }
    }

    /// Add two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.wrapping_arithmetic(left, right, u128::wrapping_add)
    }

    /// Add two admitted values when their mathematical sum remains admitted.
    pub fn exact_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.exact_arithmetic(left, right, u128::checked_add, |_, left, right| {
            left.checked_add(right)
        })
    }

    /// Subtract two admitted values when their mathematical difference remains admitted.
    pub fn exact_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.exact_arithmetic(left, right, u128::checked_sub, |_, left, right| {
            left.checked_sub(right)
        })
    }

    /// Multiply two admitted values when their mathematical product remains admitted.
    pub fn exact_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.exact_arithmetic(left, right, u128::checked_mul, |_, left, right| {
            left.checked_mul(right)
        })
    }

    /// Divide two admitted values with truncation toward zero when the divisor
    /// is nonzero and the mathematical quotient remains admitted.
    pub fn exact_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.exact_arithmetic(left, right, u128::checked_div, |_, left, right| {
            left.checked_div(right)
        })
    }

    /// Compute a truncating remainder when the corresponding quotient is
    /// defined and the result remains admitted.
    pub fn exact_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.exact_arithmetic(
            left,
            right,
            u128::checked_rem,
            |integer_type, left, right| {
                let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    None
                } else {
                    left.checked_rem(right)
                }
            },
        )
    }

    /// Divide two admitted values with truncation toward zero and reduce the
    /// sole signed quotient overflow modulo this integer width.
    pub fn wrapping_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.checked_quotient_arithmetic(
            left,
            right,
            u128::checked_div,
            i128::checked_div,
            self.minimum_value(),
        )
    }

    /// Compute a truncating remainder and reduce the sole signed quotient
    /// overflow to the wrapping-policy result of zero.
    pub fn wrapping_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.checked_quotient_arithmetic(
            left,
            right,
            u128::checked_rem,
            i128::checked_rem,
            IntegerValue::Signed(0),
        )
    }

    /// Divide two admitted values with truncation toward zero and clamp the
    /// sole signed quotient overflow to this integer width's maximum.
    pub fn saturating_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.checked_quotient_arithmetic(
            left,
            right,
            u128::checked_div,
            i128::checked_div,
            self.maximum_value(),
        )
    }

    /// Compute a truncating remainder and reduce the sole signed quotient
    /// overflow to the saturating-policy result of zero.
    pub fn saturating_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.checked_quotient_arithmetic(
            left,
            right,
            u128::checked_rem,
            i128::checked_rem,
            IntegerValue::Signed(0),
        )
    }

    /// Subtract two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.wrapping_arithmetic(left, right, u128::wrapping_sub)
    }

    /// Multiply two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.wrapping_arithmetic(left, right, u128::wrapping_mul)
    }

    /// Shift an admitted value left after reducing the count modulo this
    /// value type's exact width.
    ///
    /// The count retains its own signedness and width. Signed negative counts
    /// use Euclidean reduction, so current power-of-two source widths agree
    /// exactly with masking the count's two's-complement representation by
    /// `width - 1` while arbitrary terminal widths retain a coherent modular
    /// meaning.
    pub fn wrapping_shift_left(
        self,
        value: IntegerValue,
        count_type: IntegerType,
        count: IntegerValue,
    ) -> Option<IntegerValue> {
        if !self.admits(value) || !count_type.admits(count) {
            return None;
        }
        let count = wrapping_shift_count(self.bits, count)?;
        let mask = self.bit_mask();
        match (self.sign, value) {
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                Some(IntegerValue::Unsigned(value.wrapping_shl(count) & mask))
            }
            (IntegerSign::Signed, IntegerValue::Signed(value)) => Some(IntegerValue::Signed(
                self.signed_from_bits((value as u128).wrapping_shl(count) & mask),
            )),
            _ => None,
        }
    }

    /// Shift an admitted value right after reducing the count modulo this
    /// value type's exact width. Unsigned values zero-fill; signed values
    /// sign-fill.
    pub fn wrapping_shift_right(
        self,
        value: IntegerValue,
        count_type: IntegerType,
        count: IntegerValue,
    ) -> Option<IntegerValue> {
        if !self.admits(value) || !count_type.admits(count) {
            return None;
        }
        let count = wrapping_shift_count(self.bits, count)?;
        match (self.sign, value) {
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                Some(IntegerValue::Unsigned(value >> count))
            }
            (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                Some(IntegerValue::Signed(value >> count))
            }
            _ => None,
        }
    }

    /// Shift an admitted value right by an independently typed count whose
    /// mathematical value is inside `[0, width)`. Unlike the wrapping form,
    /// an out-of-range or negative count has no exact value.
    pub fn exact_shift_right(
        self,
        value: IntegerValue,
        count_type: IntegerType,
        count: IntegerValue,
    ) -> Option<IntegerValue> {
        if !self.admits(value) || !count_type.admits(count) {
            return None;
        }
        let count = match count {
            IntegerValue::Unsigned(count) if count < u128::from(self.bits) => {
                u32::try_from(count).ok()?
            }
            IntegerValue::Signed(count)
                if count >= 0 && (count as u128) < u128::from(self.bits) =>
            {
                u32::try_from(count).ok()?
            }
            _ => return None,
        };
        match (self.sign, value) {
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                Some(IntegerValue::Unsigned(value >> count))
            }
            (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                Some(IntegerValue::Signed(value >> count))
            }
            _ => None,
        }
    }

    /// Shift an admitted value left by an independently typed count whose
    /// mathematical value is inside `[0, width)`, rejecting any mathematical
    /// result outside this value carrier.
    pub fn exact_shift_left(
        self,
        value: IntegerValue,
        count_type: IntegerType,
        count: IntegerValue,
    ) -> Option<IntegerValue> {
        if !self.admits(value) || !count_type.admits(count) {
            return None;
        }
        let count = match count {
            IntegerValue::Unsigned(count) if count < u128::from(self.bits) => {
                u32::try_from(count).ok()?
            }
            IntegerValue::Signed(count)
                if count >= 0 && (count as u128) < u128::from(self.bits) =>
            {
                u32::try_from(count).ok()?
            }
            _ => return None,
        };
        match (self.sign, value) {
            (IntegerSign::Unsigned, IntegerValue::Unsigned(value)) => {
                let maximum = match self.maximum_value() {
                    IntegerValue::Unsigned(maximum) => maximum,
                    IntegerValue::Signed(_) => unreachable!("unsigned type has unsigned maximum"),
                };
                (value <= (maximum >> count))
                    .then_some(IntegerValue::Unsigned(value.wrapping_shl(count)))
            }
            (IntegerSign::Signed, IntegerValue::Signed(value)) => {
                let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                    (self.minimum_value(), self.maximum_value())
                else {
                    unreachable!("signed type has signed bounds")
                };
                if value < (minimum >> count) || value > (maximum >> count) {
                    return None;
                }
                Some(IntegerValue::Signed(self.signed_from_bits(
                    (value as u128).wrapping_shl(count) & self.bit_mask(),
                )))
            }
            _ => None,
        }
    }

    fn bit_mask(self) -> u128 {
        if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        }
    }

    fn signed_from_bits(self, bits: u128) -> i128 {
        let mask = self.bit_mask();
        if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
            bits as i128
        } else {
            (bits | !mask) as i128
        }
    }

    /// One two's-complement modular binary operation over two admitted
    /// values: unsigned lanes mask the raw `u128` result; signed lanes mask
    /// then re-interpret the reduced bit pattern through `signed_from_bits`.
    fn wrapping_arithmetic(
        self,
        left: IntegerValue,
        right: IntegerValue,
        operation: fn(u128, u128) -> u128,
    ) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let mask = self.bit_mask();
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(operation(left, right) & mask)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                Some(IntegerValue::Signed(self.signed_from_bits(
                    operation(left as u128, right as u128) & mask,
                )))
            }
            _ => None,
        }
    }

    /// One checked binary operation over two admitted values whose
    /// mathematical result must itself remain admitted; `signed` may consult
    /// the type for carrier-specific overflow guards.
    fn exact_arithmetic(
        self,
        left: IntegerValue,
        right: IntegerValue,
        unsigned: fn(u128, u128) -> Option<u128>,
        signed: impl FnOnce(IntegerType, i128, i128) -> Option<i128>,
    ) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let result = match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => IntegerValue::Unsigned(unsigned(left, right)?),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                IntegerValue::Signed(signed(self, left, right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    /// One checked quotient or remainder over two admitted values: the sole
    /// signed `minimum / -1` overflow reduces to `overflow` under the calling
    /// policy, while every defined result stays admission-checked.
    fn checked_quotient_arithmetic(
        self,
        left: IntegerValue,
        right: IntegerValue,
        unsigned: fn(u128, u128) -> Option<u128>,
        signed: fn(i128, i128) -> Option<i128>,
        overflow: IntegerValue,
    ) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(unsigned(left, right)?)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let IntegerValue::Signed(minimum) = self.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    Some(overflow)
                } else {
                    let result = IntegerValue::Signed(signed(left, right)?);
                    self.admits(result).then_some(result)
                }
            }
            _ => None,
        }
    }

    /// One saturating binary operation over two admitted values: unsigned
    /// lanes clamp at this width's maximum; signed lanes clamp at this
    /// width's bounds, leaving native width-128 saturation to stand.
    fn saturating_arithmetic(
        self,
        left: IntegerValue,
        right: IntegerValue,
        unsigned: fn(u128, u128) -> u128,
        signed: fn(i128, i128) -> i128,
    ) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(
                unsigned(left, right).min(self.bit_mask()),
            )),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let value = if self.bits == 128 {
                    signed(left, right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    signed(left, right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Add two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.saturating_arithmetic(left, right, u128::saturating_add, i128::saturating_add)
    }

    /// Subtract two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.saturating_arithmetic(left, right, u128::saturating_sub, i128::saturating_sub)
    }

    /// Multiply two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.saturating_arithmetic(left, right, u128::saturating_mul, i128::saturating_mul)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerValue {
    Signed(i128),
    Unsigned(u128),
}

/// One canonical mathematical integer literal. Unlike [`IntegerValue`], this
/// proof-only form carries no fixed-width signedness and therefore has exactly
/// one representation for every integer in its supported literal range.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerMathLiteral {
    negative: bool,
    magnitude: u128,
}

impl IntegerMathLiteral {
    pub fn new(negative: bool, magnitude: u128) -> Result<Self, PropositionError> {
        if negative && magnitude == 0 {
            return Err(PropositionError::NegativeZeroIntegerMathLiteral);
        }
        Ok(Self {
            negative,
            magnitude,
        })
    }

    pub const fn negative(self) -> bool {
        self.negative
    }

    pub const fn magnitude(self) -> u128 {
        self.magnitude
    }

    pub const fn from_integer_value(value: IntegerValue) -> Self {
        match value {
            IntegerValue::Signed(value) => Self {
                negative: value.is_negative(),
                magnitude: value.unsigned_abs(),
            },
            IntegerValue::Unsigned(value) => Self {
                negative: false,
                magnitude: value,
            },
        }
    }

    pub fn as_integer_value(self, integer_type: IntegerType) -> Option<IntegerValue> {
        let value = match integer_type.sign() {
            IntegerSign::Signed if self.negative => {
                if self.magnitude == (i128::MAX as u128) + 1 {
                    IntegerValue::Signed(i128::MIN)
                } else {
                    IntegerValue::Signed(-i128::try_from(self.magnitude).ok()?)
                }
            }
            IntegerSign::Signed => IntegerValue::Signed(i128::try_from(self.magnitude).ok()?),
            IntegerSign::Unsigned if !self.negative => IntegerValue::Unsigned(self.magnitude),
            IntegerSign::Unsigned => return None,
        };
        integer_type.admits(value).then_some(value)
    }
}

/// Total, proof-only mathematical integer syntax. `MathValue` embeds the
/// mathematical value of one fixed-width integer carrier; the other forms are
/// interpreted over unbounded integers rather than machine arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerMathTerm {
    IntegerLiteral(IntegerMathLiteral),
    MathValue {
        source_type: IntegerType,
        value: ValueId,
    },
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    ShiftLeft {
        value: Box<Self>,
        count: Box<Self>,
    },
}

impl IntegerMathTerm {
    pub fn literal(value: IntegerValue) -> Self {
        Self::IntegerLiteral(IntegerMathLiteral::from_integer_value(value))
    }

    pub fn validate(&self) -> Result<(), PropositionError> {
        // An explicit worklist keeps deep terms off the call stack: children
        // push in reverse so the left operand still validates first, matching
        // the error order the recursive validator produced.
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                Self::MathValue { source_type, .. } => {
                    if source_type.is_address() {
                        return Err(PropositionError::AddressIntegerMathValue(*source_type));
                    }
                }
                Self::IntegerLiteral(literal) => {
                    if literal.negative && literal.magnitude == 0 {
                        return Err(PropositionError::NegativeZeroIntegerMathLiteral);
                    }
                }
                Self::Add(left, right)
                | Self::Subtract(left, right)
                | Self::Multiply(left, right) => {
                    pending.push(right);
                    pending.push(left);
                }
                Self::ShiftLeft { value, count } => {
                    pending.push(count);
                    pending.push(value);
                }
            }
        }
        Ok(())
    }
}

fn wrapping_shift_count(width: u16, count: IntegerValue) -> Option<u32> {
    let width = u128::from(width);
    match count {
        IntegerValue::Unsigned(count) => u32::try_from(count % width).ok(),
        IntegerValue::Signed(count) => {
            let width = i128::try_from(width).ok()?;
            u32::try_from(count.rem_euclid(width)).ok()
        }
    }
}
