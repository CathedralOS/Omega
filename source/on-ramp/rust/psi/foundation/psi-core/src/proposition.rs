use std::{cmp::Ordering, collections::BTreeMap};

use crate::{
    ContentConservation, ContentPlaceVersion, ContentTerm, PlaceId, PropositionId,
    StructuralCaseId, StructuralFieldId, StructuralPlaceKind, ValueId,
};

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
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let mask = if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        };
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.wrapping_add(right) & mask)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let bits = (left as u128).wrapping_add(right as u128) & mask;
                let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                    bits as i128
                } else {
                    (bits | !mask) as i128
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Add two admitted values when their mathematical sum remains admitted.
    pub fn exact_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let result = match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => IntegerValue::Unsigned(left.checked_add(right)?),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                IntegerValue::Signed(left.checked_add(right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    /// Subtract two admitted values when their mathematical difference remains admitted.
    pub fn exact_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let result = match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => IntegerValue::Unsigned(left.checked_sub(right)?),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                IntegerValue::Signed(left.checked_sub(right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    /// Multiply two admitted values when their mathematical product remains admitted.
    pub fn exact_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let result = match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => IntegerValue::Unsigned(left.checked_mul(right)?),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                IntegerValue::Signed(left.checked_mul(right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    /// Divide two admitted values with truncation toward zero when the divisor
    /// is nonzero and the mathematical quotient remains admitted.
    pub fn exact_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let result = match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => IntegerValue::Unsigned(left.checked_div(right)?),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                IntegerValue::Signed(left.checked_div(right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    /// Compute a truncating remainder when the corresponding quotient is
    /// defined and the result remains admitted.
    pub fn exact_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let result = match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => IntegerValue::Unsigned(left.checked_rem(right)?),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let IntegerValue::Signed(minimum) = self.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    return None;
                }
                IntegerValue::Signed(left.checked_rem(right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    /// Divide two admitted values with truncation toward zero and reduce the
    /// sole signed quotient overflow modulo this integer width.
    pub fn wrapping_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.checked_div(right)?)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let IntegerValue::Signed(minimum) = self.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    Some(IntegerValue::Signed(minimum))
                } else {
                    let result = IntegerValue::Signed(left.checked_div(right)?);
                    self.admits(result).then_some(result)
                }
            }
            _ => None,
        }
    }

    /// Compute a truncating remainder and reduce the sole signed quotient
    /// overflow to the wrapping-policy result of zero.
    pub fn wrapping_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.checked_rem(right)?)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let IntegerValue::Signed(minimum) = self.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    Some(IntegerValue::Signed(0))
                } else {
                    let result = IntegerValue::Signed(left.checked_rem(right)?);
                    self.admits(result).then_some(result)
                }
            }
            _ => None,
        }
    }

    /// Divide two admitted values with truncation toward zero and clamp the
    /// sole signed quotient overflow to this integer width's maximum.
    pub fn saturating_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.checked_div(right)?)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let IntegerValue::Signed(minimum) = self.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    Some(self.maximum_value())
                } else {
                    let result = IntegerValue::Signed(left.checked_div(right)?);
                    self.admits(result).then_some(result)
                }
            }
            _ => None,
        }
    }

    /// Compute a truncating remainder and reduce the sole signed quotient
    /// overflow to the saturating-policy result of zero.
    pub fn saturating_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.checked_rem(right)?)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let IntegerValue::Signed(minimum) = self.minimum_value() else {
                    unreachable!("signed type has a signed minimum")
                };
                if left == minimum && right == -1 {
                    Some(IntegerValue::Signed(0))
                } else {
                    let result = IntegerValue::Signed(left.checked_rem(right)?);
                    self.admits(result).then_some(result)
                }
            }
            _ => None,
        }
    }

    /// Subtract two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let mask = if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        };
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.wrapping_sub(right) & mask)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let bits = (left as u128).wrapping_sub(right as u128) & mask;
                let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                    bits as i128
                } else {
                    (bits | !mask) as i128
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Multiply two admitted values modulo this exact integer width.
    ///
    /// Signed results use two's-complement interpretation of the reduced bit
    /// pattern. A sign/value mismatch or out-of-range input is rejected rather
    /// than silently reinterpreted.
    pub fn wrapping_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        let mask = if self.bits == 128 {
            u128::MAX
        } else {
            (1_u128 << self.bits) - 1
        };
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.wrapping_mul(right) & mask)),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let bits = (left as u128).wrapping_mul(right as u128) & mask;
                let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                    bits as i128
                } else {
                    (bits | !mask) as i128
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
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

    /// Add two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => {
                let maximum = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                Some(IntegerValue::Unsigned(
                    left.saturating_add(right).min(maximum),
                ))
            }
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let value = if self.bits == 128 {
                    left.saturating_add(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    (left + right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Subtract two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => Some(IntegerValue::Unsigned(left.saturating_sub(right))),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let value = if self.bits == 128 {
                    left.saturating_sub(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    (left - right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }

    /// Multiply two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        if !self.admits(left) || !self.admits(right) {
            return None;
        }
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => {
                let maximum = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                Some(IntegerValue::Unsigned(
                    left.saturating_mul(right).min(maximum),
                ))
            }
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                let value = if self.bits == 128 {
                    left.saturating_mul(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    left.saturating_mul(right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            }
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerValue {
    Signed(i128),
    Unsigned(u128),
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarType {
    Boolean,
    Integer(IntegerType),
}

/// One canonical step below a terminal structural root in a scalar
/// proposition. Field steps use verifier-owned structural-field identities;
/// fixed indices retain the exact literal array element; case steps enter one
/// exact sum payload namespace before a following field step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum CanonicalStructuralPathSegment {
    Field(StructuralFieldId),
    FixedIndex(u64),
    Case(StructuralCaseId),
}

/// Exact IEEE interchange format retained by target-neutral structural
/// predicates. This is deliberately separate from [`ScalarType`]: the current
/// Terminal execution vocabulary does not claim general floating-point scalar
/// evaluation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatFormat {
    Binary32,
    Binary64,
}

/// Source IEEE comparison retained without mathematical-equality laws.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IeeeFloatComparisonKind {
    Equal,
    NotEqual,
}

/// One nonempty canonical path to a relevant IEEE floating-point field below
/// a Terminal structural root.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IeeeFloatStructuralField {
    root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
}

impl IeeeFloatStructuralField {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<Self, PropositionError> {
        if path.is_empty() {
            return Err(PropositionError::EmptyIeeeFloatStructuralFieldPath);
        }
        Ok(Self { root, path })
    }

    pub const fn root(&self) -> PlaceId {
        self.root
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self { root, path }
    }
}

/// One nonempty canonical path to a byte-sequence field below a Terminal
/// structural root. Equality observes only the live length and byte prefix;
/// native descriptor identity and unused bounded capacity are not semantic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ByteSequenceStructuralField {
    root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
}

/// One structural subject whose active sum case can be observed.
/// The path may be empty when the subject is the structural root itself.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct StructuralCaseSubject {
    root: PlaceId,
    path: Vec<CanonicalStructuralPathSegment>,
}

impl StructuralCaseSubject {
    pub fn new(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self { root, path }
    }

    pub const fn root(&self) -> PlaceId {
        self.root
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self { root, path }
    }
}

impl ByteSequenceStructuralField {
    pub fn new(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    ) -> Result<Self, PropositionError> {
        if path.is_empty() {
            return Err(PropositionError::EmptyByteSequenceStructuralFieldPath);
        }
        Ok(Self { root, path })
    }

    pub const fn root(&self) -> PlaceId {
        self.root
    }

    pub fn path(&self) -> &[CanonicalStructuralPathSegment] {
        &self.path
    }

    pub fn rebase(&self, root: PlaceId, prefix: &[CanonicalStructuralPathSegment]) -> Self {
        let mut path = Vec::with_capacity(prefix.len() + self.path.len());
        path.extend_from_slice(prefix);
        path.extend_from_slice(&self.path);
        Self { root, path }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarTerm {
    Value {
        id: ValueId,
        scalar_type: ScalarType,
    },
    /// One nonempty canonical structural path to a relevant Boolean field
    /// below a terminal structural root. The terminal verifier traverses every
    /// field and fixed index against the exact declared structural types and
    /// independently confirms that the final field is Boolean.
    BooleanField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
    },
    /// One nonempty canonical structural path to a relevant fixed-integer
    /// field. The repeated integer type is independently checked against the
    /// terminal structural declaration at the leaf.
    IntegerField {
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    },
    Boolean(bool),
    BooleanNot {
        operand: Box<ScalarTerm>,
    },
    BooleanEqual {
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerEqual {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerLessThan {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerLessOrEqual {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseAnd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseNot {
        scalar_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerWiden {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerExactCast {
        source_type: IntegerType,
        target_type: IntegerType,
        operand: Box<ScalarTerm>,
    },
    IntegerBitwiseOr {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    IntegerBitwiseXor {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerShiftLeft {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    WrappingIntegerShiftRight {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerShiftLeft {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerShiftRight {
        value_type: IntegerType,
        count_type: IntegerType,
        value: Box<ScalarTerm>,
        count: Box<ScalarTerm>,
    },
    ExactIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    ExactIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerDivide {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerRemainder {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    Integer {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    WrappingIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerAdd {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerSubtract {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    WrappingIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
    SaturatingIntegerMultiply {
        scalar_type: IntegerType,
        left: Box<ScalarTerm>,
        right: Box<ScalarTerm>,
    },
}

impl ScalarTerm {
    pub fn value(id: ValueId, scalar_type: ScalarType) -> Self {
        Self::Value { id, scalar_type }
    }

    pub fn boolean_field(root: PlaceId, field: StructuralFieldId) -> Self {
        Self::boolean_field_path(root, vec![CanonicalStructuralPathSegment::Field(field)])
    }

    pub fn boolean_field_path(root: PlaceId, path: Vec<CanonicalStructuralPathSegment>) -> Self {
        Self::BooleanField { root, path }
    }

    pub fn integer_field_path(
        root: PlaceId,
        path: Vec<CanonicalStructuralPathSegment>,
        scalar_type: IntegerType,
    ) -> Self {
        Self::IntegerField {
            root,
            path,
            scalar_type,
        }
    }

    pub const fn boolean(value: bool) -> Self {
        Self::Boolean(value)
    }

    pub fn boolean_not(operand: ScalarTerm) -> Result<Self, PropositionError> {
        if operand.scalar_type() != ScalarType::Boolean {
            return Err(PropositionError::BooleanNotTypeMismatch(
                operand.scalar_type(),
            ));
        }
        Ok(Self::BooleanNot {
            operand: Box::new(operand),
        })
    }

    pub fn boolean_equal(left: ScalarTerm, right: ScalarTerm) -> Result<Self, PropositionError> {
        if left.scalar_type() != ScalarType::Boolean || right.scalar_type() != ScalarType::Boolean {
            return Err(PropositionError::BooleanEqualTypeMismatch {
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::BooleanEqual {
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_equal(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::IntegerEqualTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::IntegerEqual {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_less_than(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerLessThan {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_less_or_equal(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerLessOrEqual {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_bitwise_and(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerBitwiseAnd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_bitwise_not(
        scalar_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if operand.scalar_type() != expected {
            return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                expected,
                operand: operand.scalar_type(),
            });
        }
        Ok(Self::IntegerBitwiseNot {
            scalar_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_widen(
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let actual = operand.scalar_type();
        let expected = ScalarType::Integer(source_type);
        if actual != expected || !source_type.can_widen_to(target_type) {
            return Err(PropositionError::IntegerWidenTypeMismatch {
                source: expected,
                target: ScalarType::Integer(target_type),
                operand: actual,
            });
        }
        Ok(Self::IntegerWiden {
            source_type,
            target_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_exact_cast(
        source_type: IntegerType,
        target_type: IntegerType,
        operand: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let actual = operand.scalar_type();
        let expected = ScalarType::Integer(source_type);
        if actual != expected || !source_type.can_exact_cast_to(target_type) {
            return Err(PropositionError::IntegerExactCastTypeMismatch {
                source: expected,
                target: ScalarType::Integer(target_type),
                operand: actual,
            });
        }
        Ok(Self::IntegerExactCast {
            source_type,
            target_type,
            operand: Box::new(operand),
        })
    }

    pub fn integer_bitwise_or(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerBitwiseOr {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer_bitwise_xor(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::IntegerBitwiseXor {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_shift_left(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::WrappingIntegerShiftLeft {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn wrapping_integer_shift_right(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::WrappingIntegerShiftRight {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn exact_integer_shift_right(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::ExactIntegerShiftRight {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn exact_integer_shift_left(
        value_type: IntegerType,
        count_type: IntegerType,
        value: ScalarTerm,
        count: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_shift_operands(value_type, count_type, &value, &count)?;
        Ok(Self::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value: Box::new(value),
            count: Box::new(count),
        })
    }

    pub fn exact_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_multiply(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_divide(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerDivide {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn exact_integer_remainder(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::ExactIntegerRemainder {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_divide(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::WrappingIntegerDivide {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_remainder(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::WrappingIntegerRemainder {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_divide(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::SaturatingIntegerDivide {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_remainder(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        validate_integer_operands(scalar_type, &left, &right)?;
        Ok(Self::SaturatingIntegerRemainder {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn integer(
        scalar_type: IntegerType,
        value: IntegerValue,
    ) -> Result<Self, PropositionError> {
        if !scalar_type.admits(value) {
            return Err(PropositionError::IntegerLiteralOutsideType { scalar_type, value });
        }
        Ok(Self::Integer { scalar_type, value })
    }

    pub fn wrapping_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::WrappingIntegerAddTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::WrappingIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_add(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::SaturatingIntegerAddTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::SaturatingIntegerAdd {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::WrappingIntegerSubtractTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::WrappingIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_subtract(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::SaturatingIntegerSubtractTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::SaturatingIntegerSubtract {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn wrapping_integer_multiply(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::WrappingIntegerMultiplyTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::WrappingIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn saturating_integer_multiply(
        scalar_type: IntegerType,
        left: ScalarTerm,
        right: ScalarTerm,
    ) -> Result<Self, PropositionError> {
        let expected = ScalarType::Integer(scalar_type);
        if left.scalar_type() != expected || right.scalar_type() != expected {
            return Err(PropositionError::SaturatingIntegerMultiplyTypeMismatch {
                expected,
                left: left.scalar_type(),
                right: right.scalar_type(),
            });
        }
        Ok(Self::SaturatingIntegerMultiply {
            scalar_type,
            left: Box::new(left),
            right: Box::new(right),
        })
    }

    pub fn scalar_type(&self) -> ScalarType {
        match self {
            Self::Value { scalar_type, .. } => *scalar_type,
            Self::IntegerField { scalar_type, .. } => ScalarType::Integer(*scalar_type),
            Self::Boolean(_)
            | Self::BooleanField { .. }
            | Self::BooleanNot { .. }
            | Self::BooleanEqual { .. }
            | Self::IntegerEqual { .. }
            | Self::IntegerLessThan { .. }
            | Self::IntegerLessOrEqual { .. } => ScalarType::Boolean,
            Self::Integer { scalar_type, .. }
            | Self::IntegerBitwiseNot { scalar_type, .. }
            | Self::IntegerBitwiseAnd { scalar_type, .. }
            | Self::IntegerBitwiseOr { scalar_type, .. }
            | Self::IntegerBitwiseXor { scalar_type, .. }
            | Self::ExactIntegerAdd { scalar_type, .. }
            | Self::ExactIntegerSubtract { scalar_type, .. }
            | Self::ExactIntegerMultiply { scalar_type, .. }
            | Self::ExactIntegerDivide { scalar_type, .. }
            | Self::ExactIntegerRemainder { scalar_type, .. }
            | Self::WrappingIntegerDivide { scalar_type, .. }
            | Self::WrappingIntegerRemainder { scalar_type, .. }
            | Self::SaturatingIntegerDivide { scalar_type, .. }
            | Self::SaturatingIntegerRemainder { scalar_type, .. }
            | Self::WrappingIntegerAdd { scalar_type, .. }
            | Self::SaturatingIntegerAdd { scalar_type, .. }
            | Self::WrappingIntegerSubtract { scalar_type, .. }
            | Self::SaturatingIntegerSubtract { scalar_type, .. }
            | Self::WrappingIntegerMultiply { scalar_type, .. }
            | Self::SaturatingIntegerMultiply { scalar_type, .. } => {
                ScalarType::Integer(*scalar_type)
            }
            Self::IntegerWiden { target_type, .. } | Self::IntegerExactCast { target_type, .. } => {
                ScalarType::Integer(*target_type)
            }
            Self::WrappingIntegerShiftLeft { value_type, .. }
            | Self::WrappingIntegerShiftRight { value_type, .. }
            | Self::ExactIntegerShiftLeft { value_type, .. }
            | Self::ExactIntegerShiftRight { value_type, .. } => ScalarType::Integer(*value_type),
        }
    }

    pub fn integer_value(&self) -> Option<(IntegerType, IntegerValue)> {
        match self {
            Self::Integer { scalar_type, value } => Some((*scalar_type, *value)),
            Self::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                let (operand_type, operand) = operand.integer_value()?;
                if operand_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.bitwise_not(operand)?))
            }
            Self::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => {
                let (operand_type, operand) = operand.integer_value()?;
                if operand_type != *source_type || !source_type.can_widen_to(*target_type) {
                    return None;
                }
                Some((
                    *target_type,
                    source_type.widen_value_to(*target_type, operand)?,
                ))
            }
            Self::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => {
                let (operand_type, operand) = operand.integer_value()?;
                if operand_type != *source_type || !source_type.can_exact_cast_to(*target_type) {
                    return None;
                }
                Some((
                    *target_type,
                    source_type.exact_cast_value_to(*target_type, operand)?,
                ))
            }
            Self::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_add(left, right)?))
            }
            Self::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_add(left, right)?))
            }
            Self::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_sub(left, right)?))
            }
            Self::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_sub(left, right)?))
            }
            Self::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_mul(left, right)?))
            }
            Self::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_mul(left, right)?))
            }
            Self::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                let value = match self {
                    Self::IntegerBitwiseAnd { .. } => scalar_type.bitwise_and(left, right)?,
                    Self::IntegerBitwiseOr { .. } => scalar_type.bitwise_or(left, right)?,
                    Self::IntegerBitwiseXor { .. } => scalar_type.bitwise_xor(left, right)?,
                    _ => unreachable!(),
                };
                Some((*scalar_type, value))
            }
            Self::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                let (actual_value_type, value) = value.integer_value()?;
                let (actual_count_type, count) = count.integer_value()?;
                if actual_value_type != *value_type || actual_count_type != *count_type {
                    return None;
                }
                let result = match self {
                    Self::WrappingIntegerShiftLeft { .. } => {
                        value_type.wrapping_shift_left(value, *count_type, count)?
                    }
                    Self::WrappingIntegerShiftRight { .. } => {
                        value_type.wrapping_shift_right(value, *count_type, count)?
                    }
                    Self::ExactIntegerShiftLeft { .. } => {
                        value_type.exact_shift_left(value, *count_type, count)?
                    }
                    Self::ExactIntegerShiftRight { .. } => {
                        value_type.exact_shift_right(value, *count_type, count)?
                    }
                    _ => unreachable!(),
                };
                Some((*value_type, result))
            }
            Self::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_add(left, right)?))
            }
            Self::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_sub(left, right)?))
            }
            Self::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_mul(left, right)?))
            }
            Self::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_div(left, right)?))
            }
            Self::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.exact_rem(left, right)?))
            }
            Self::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_div(left, right)?))
            }
            Self::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.wrapping_rem(left, right)?))
            }
            Self::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_div(left, right)?))
            }
            Self::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some((*scalar_type, scalar_type.saturating_rem(left, right)?))
            }
            _ => None,
        }
    }

    pub fn boolean_value(&self) -> Option<bool> {
        match self {
            Self::Boolean(value) => Some(*value),
            Self::BooleanNot { operand } => Some(!operand.boolean_value()?),
            Self::BooleanEqual { left, right } => {
                Some(left.boolean_value()? == right.boolean_value()?)
            }
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                (left_type == *scalar_type && right_type == *scalar_type).then_some(left == right)
            }
            Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some(scalar_type.compare(left, right)?.is_lt())
            }
            Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => {
                let (left_type, left) = left.integer_value()?;
                let (right_type, right) = right.integer_value()?;
                if left_type != *scalar_type || right_type != *scalar_type {
                    return None;
                }
                Some(!scalar_type.compare(left, right)?.is_gt())
            }
            _ => None,
        }
    }

    pub fn validate(&self) -> Result<(), PropositionError> {
        match self {
            Self::Value { .. }
            | Self::BooleanField { .. }
            | Self::IntegerField { .. }
            | Self::Boolean(_) => Ok(()),
            Self::BooleanNot { operand } => {
                operand.validate()?;
                if operand.scalar_type() != ScalarType::Boolean {
                    return Err(PropositionError::BooleanNotTypeMismatch(
                        operand.scalar_type(),
                    ));
                }
                Ok(())
            }
            Self::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } => {
                operand.validate()?;
                let expected = ScalarType::Integer(*source_type);
                if operand.scalar_type() != expected || !source_type.can_exact_cast_to(*target_type)
                {
                    return Err(PropositionError::IntegerExactCastTypeMismatch {
                        source: expected,
                        target: ScalarType::Integer(*target_type),
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::BooleanEqual { left, right } => {
                left.validate()?;
                right.validate()?;
                if left.scalar_type() != ScalarType::Boolean
                    || right.scalar_type() != ScalarType::Boolean
                {
                    return Err(PropositionError::BooleanEqualTypeMismatch {
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerEqual {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::IntegerEqualTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerLessThan {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerLessOrEqual {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::IntegerBitwiseNot {
                scalar_type,
                operand,
            } => {
                operand.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if operand.scalar_type() != expected {
                    return Err(PropositionError::IntegerBitwiseNotTypeMismatch {
                        expected,
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerWiden {
                source_type,
                target_type,
                operand,
            } => {
                operand.validate()?;
                let expected = ScalarType::Integer(*source_type);
                if operand.scalar_type() != expected || !source_type.can_widen_to(*target_type) {
                    return Err(PropositionError::IntegerWidenTypeMismatch {
                        source: expected,
                        target: ScalarType::Integer(*target_type),
                        operand: operand.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::IntegerBitwiseAnd {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseOr {
                scalar_type,
                left,
                right,
            }
            | Self::IntegerBitwiseXor {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::WrappingIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::WrappingIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | Self::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } => {
                value.validate()?;
                count.validate()?;
                validate_integer_shift_operands(*value_type, *count_type, value, count)
            }
            Self::Integer { scalar_type, value } => {
                if scalar_type.admits(*value) {
                    Ok(())
                } else {
                    Err(PropositionError::IntegerLiteralOutsideType {
                        scalar_type: *scalar_type,
                        value: *value,
                    })
                }
            }
            Self::WrappingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::WrappingIntegerAddTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::ExactIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::WrappingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::WrappingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::SaturatingIntegerDivide {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::SaturatingIntegerRemainder {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                validate_integer_operands(*scalar_type, left, right)
            }
            Self::SaturatingIntegerAdd {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::SaturatingIntegerAddTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::WrappingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::WrappingIntegerSubtractTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::SaturatingIntegerSubtract {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::SaturatingIntegerSubtractTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::WrappingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::WrappingIntegerMultiplyTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
            Self::SaturatingIntegerMultiply {
                scalar_type,
                left,
                right,
            } => {
                left.validate()?;
                right.validate()?;
                let expected = ScalarType::Integer(*scalar_type);
                if left.scalar_type() != expected || right.scalar_type() != expected {
                    return Err(PropositionError::SaturatingIntegerMultiplyTypeMismatch {
                        expected,
                        left: left.scalar_type(),
                        right: right.scalar_type(),
                    });
                }
                Ok(())
            }
        }
    }
}

/// The initial terminal-Psi proposition vocabulary.
///
/// This is intentionally small but not source-shaped. It refers only to
/// stable terminal values or closed literals. Operation-specific predicates
/// will extend this vocabulary together with their execution semantics and
/// obligation schemas.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Proposition {
    Truth,
    Falsehood,
    Atom(PropositionId),
    Equal(ScalarTerm, ScalarTerm),
    LessThan(ScalarTerm, ScalarTerm),
    LessOrEqual(ScalarTerm, ScalarTerm),
    /// IEEE `==` or `!=` over two exact structural leaves. This remains atomic
    /// rather than using mathematical equality: NaNs are non-reflexive while
    /// signed zeroes compare equal under `==` and unequal under `!=`.
    IeeeFloatComparison {
        kind: IeeeFloatComparisonKind,
        format: IeeeFloatFormat,
        left: IeeeFloatStructuralField,
        right: IeeeFloatStructuralField,
    },
    /// Content equality over two exact byte-sequence structural leaves.
    ByteSequenceEqual {
        left: ByteSequenceStructuralField,
        right: ByteSequenceStructuralField,
    },
    /// Exact membership in one declared case of a structural sum.
    StructuralCaseMembership {
        subject: StructuralCaseSubject,
        case: StructuralCaseId,
    },
    Conjunction(Vec<Proposition>),
    Disjunction(Vec<Proposition>),
    Implication {
        premise: Box<Proposition>,
        conclusion: Box<Proposition>,
    },
    /// Exact equality in one compiler-owned content algebra. Structural-place
    /// roots and projection identities are terminal semantic identities, not
    /// source handles.
    ContentConservation(ContentConservation),
}

impl Proposition {
    pub fn validate(&self) -> Result<(), PropositionError> {
        match self {
            Self::Truth | Self::Falsehood => Ok(()),
            Self::Atom(_) => Ok(()),
            Self::Equal(left, right) => require_same_type(left, right),
            Self::LessThan(left, right) | Self::LessOrEqual(left, right) => {
                require_same_integer_type(left, right)
            }
            Self::IeeeFloatComparison { left, right, .. } => {
                if left.path.is_empty() || right.path.is_empty() {
                    return Err(PropositionError::EmptyIeeeFloatStructuralFieldPath);
                }
                if left > right {
                    return Err(PropositionError::NonCanonicalIeeeFloatComparisonOperands);
                }
                Ok(())
            }
            Self::ByteSequenceEqual { left, right } => {
                if left.path.is_empty() || right.path.is_empty() {
                    return Err(PropositionError::EmptyByteSequenceStructuralFieldPath);
                }
                if left > right {
                    return Err(PropositionError::NonCanonicalByteSequenceEqualOperands);
                }
                Ok(())
            }
            Self::StructuralCaseMembership { .. } => Ok(()),
            Self::Conjunction(conjuncts) => {
                if conjuncts.len() < 2 {
                    return Err(PropositionError::NonCanonicalConjunctionArity(
                        conjuncts.len(),
                    ));
                }
                for conjunct in conjuncts {
                    conjunct.validate()?;
                }
                Ok(())
            }
            Self::Disjunction(disjuncts) => {
                if disjuncts.len() < 2 {
                    return Err(PropositionError::NonCanonicalDisjunctionArity(
                        disjuncts.len(),
                    ));
                }
                for disjunct in disjuncts {
                    disjunct.validate()?;
                }
                Ok(())
            }
            Self::Implication {
                premise,
                conclusion,
            } => {
                premise.validate()?;
                conclusion.validate()
            }
            Self::ContentConservation(conservation) => conservation.validate(),
        }
    }
}

/// Terminal-module typing context used by the proof kernel.
///
/// Value terms repeat their scalar type so propositions remain locally
/// inspectable, but the kernel checks that annotation against this unique
/// module-owned table. A proof cannot reinterpret one stable value identity at
/// another type.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropositionContext {
    value_types: BTreeMap<ValueId, ScalarType>,
    structural_places: BTreeMap<PlaceId, StructuralPlaceKind>,
}

impl PropositionContext {
    pub fn from_value_types(
        value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
    ) -> Result<Self, PropositionError> {
        Self::from_value_types_and_places(value_types, [])
    }

    pub fn from_value_types_and_places(
        value_types: impl IntoIterator<Item = (ValueId, ScalarType)>,
        structural_places: impl IntoIterator<Item = (PlaceId, StructuralPlaceKind)>,
    ) -> Result<Self, PropositionError> {
        let mut context = Self::default();
        for (id, scalar_type) in value_types {
            if let Some(previous) = context.value_types.insert(id, scalar_type)
                && previous != scalar_type
            {
                return Err(PropositionError::ConflictingValueType {
                    id,
                    first: previous,
                    second: scalar_type,
                });
            }
        }
        for (id, kind) in structural_places {
            if let Some(previous) = context.structural_places.insert(id, kind)
                && previous != kind
            {
                return Err(PropositionError::ConflictingStructuralPlaceKind {
                    id,
                    first: previous,
                    second: kind,
                });
            }
        }
        Ok(context)
    }

    pub fn validate(&self, proposition: &Proposition) -> Result<(), PropositionError> {
        proposition.validate()?;
        self.validate_value_terms(proposition)
    }

    fn validate_value_terms(&self, proposition: &Proposition) -> Result<(), PropositionError> {
        match proposition {
            Proposition::Truth | Proposition::Falsehood | Proposition::Atom(_) => Ok(()),
            Proposition::Equal(left, right)
            | Proposition::LessThan(left, right)
            | Proposition::LessOrEqual(left, right) => {
                self.validate_term(left)?;
                self.validate_term(right)
            }
            Proposition::IeeeFloatComparison { left, right, .. } => {
                for field in [left, right] {
                    if !self.structural_places.contains_key(&field.root) {
                        return Err(PropositionError::UnknownStructuralPlace(field.root));
                    }
                }
                Ok(())
            }
            Proposition::ByteSequenceEqual { left, right } => {
                for field in [left, right] {
                    if !self.structural_places.contains_key(&field.root) {
                        return Err(PropositionError::UnknownStructuralPlace(field.root));
                    }
                }
                Ok(())
            }
            Proposition::StructuralCaseMembership { subject, .. } => {
                if !self.structural_places.contains_key(&subject.root) {
                    return Err(PropositionError::UnknownStructuralPlace(subject.root));
                }
                Ok(())
            }
            Proposition::Conjunction(propositions) | Proposition::Disjunction(propositions) => {
                for proposition in propositions {
                    self.validate_value_terms(proposition)?;
                }
                Ok(())
            }
            Proposition::Implication {
                premise,
                conclusion,
            } => {
                self.validate_value_terms(premise)?;
                self.validate_value_terms(conclusion)
            }
            Proposition::ContentConservation(conservation) => {
                self.validate_content_term(conservation.left())?;
                self.validate_content_term(conservation.right())
            }
        }
    }

    fn validate_content_term(&self, term: &ContentTerm) -> Result<(), PropositionError> {
        match term {
            ContentTerm::Projection { subject, .. } => {
                let Some(kind) = self.structural_places.get(&subject.root) else {
                    return Err(PropositionError::UnknownStructuralPlace(subject.root));
                };
                if subject.version == ContentPlaceVersion::Entry
                    && matches!(
                        kind,
                        StructuralPlaceKind::Result | StructuralPlaceKind::OperationResult { .. }
                    )
                {
                    return Err(PropositionError::EntryResultStructuralPlace(subject.root));
                }
                if matches!(kind, StructuralPlaceKind::TrivialAffineLocal { .. }) {
                    return Err(PropositionError::UnsupportedContentLocalStructuralPlace(
                        subject.root,
                    ));
                }
                Ok(())
            }
            ContentTerm::Separate(terms) => {
                for term in terms {
                    self.validate_content_term(term)?;
                }
                Ok(())
            }
        }
    }

    fn validate_term(&self, term: &ScalarTerm) -> Result<(), PropositionError> {
        match term {
            ScalarTerm::Value { id, scalar_type } => {
                let Some(expected) = self.value_types.get(id) else {
                    return Err(PropositionError::UnknownValue(*id));
                };
                if expected != scalar_type {
                    return Err(PropositionError::ValueTypeMismatch {
                        id: *id,
                        expected: *expected,
                        actual: *scalar_type,
                    });
                }
            }
            ScalarTerm::BooleanField { root, .. } | ScalarTerm::IntegerField { root, .. } => {
                if !self.structural_places.contains_key(root) {
                    return Err(PropositionError::UnknownStructuralPlace(*root));
                }
            }
            ScalarTerm::ExactIntegerAdd { left, right, .. }
            | ScalarTerm::ExactIntegerSubtract { left, right, .. }
            | ScalarTerm::ExactIntegerMultiply { left, right, .. }
            | ScalarTerm::ExactIntegerDivide { left, right, .. }
            | ScalarTerm::ExactIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerDivide { left, right, .. }
            | ScalarTerm::WrappingIntegerRemainder { left, right, .. }
            | ScalarTerm::SaturatingIntegerDivide { left, right, .. }
            | ScalarTerm::SaturatingIntegerRemainder { left, right, .. }
            | ScalarTerm::WrappingIntegerAdd { left, right, .. }
            | ScalarTerm::SaturatingIntegerAdd { left, right, .. }
            | ScalarTerm::WrappingIntegerSubtract { left, right, .. }
            | ScalarTerm::SaturatingIntegerSubtract { left, right, .. }
            | ScalarTerm::WrappingIntegerMultiply { left, right, .. }
            | ScalarTerm::SaturatingIntegerMultiply { left, right, .. }
            | ScalarTerm::BooleanEqual { left, right }
            | ScalarTerm::IntegerEqual { left, right, .. }
            | ScalarTerm::IntegerLessThan { left, right, .. }
            | ScalarTerm::IntegerLessOrEqual { left, right, .. }
            | ScalarTerm::IntegerBitwiseAnd { left, right, .. }
            | ScalarTerm::IntegerBitwiseOr { left, right, .. }
            | ScalarTerm::IntegerBitwiseXor { left, right, .. } => {
                self.validate_term(left)?;
                self.validate_term(right)?;
            }
            ScalarTerm::WrappingIntegerShiftLeft { value, count, .. }
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. }
            | ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                self.validate_term(value)?;
                self.validate_term(count)?;
            }
            ScalarTerm::BooleanNot { operand }
            | ScalarTerm::IntegerBitwiseNot { operand, .. }
            | ScalarTerm::IntegerWiden { operand, .. }
            | ScalarTerm::IntegerExactCast { operand, .. } => self.validate_term(operand)?,
            ScalarTerm::Boolean(_) | ScalarTerm::Integer { .. } => {}
        }
        Ok(())
    }
}

fn require_same_type(left: &ScalarTerm, right: &ScalarTerm) -> Result<(), PropositionError> {
    left.validate()?;
    right.validate()?;
    if left.scalar_type() != right.scalar_type() {
        return Err(PropositionError::MismatchedScalarTypes {
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_integer_operands(
    integer_type: IntegerType,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    let expected = ScalarType::Integer(integer_type);
    if left.scalar_type() != expected || right.scalar_type() != expected {
        return Err(PropositionError::IntegerOperandTypeMismatch {
            expected,
            left: left.scalar_type(),
            right: right.scalar_type(),
        });
    }
    Ok(())
}

fn validate_integer_shift_operands(
    value_type: IntegerType,
    count_type: IntegerType,
    value: &ScalarTerm,
    count: &ScalarTerm,
) -> Result<(), PropositionError> {
    let expected_value = ScalarType::Integer(value_type);
    let expected_count = ScalarType::Integer(count_type);
    if value.scalar_type() != expected_value || count.scalar_type() != expected_count {
        return Err(PropositionError::IntegerShiftOperandTypeMismatch {
            expected_value,
            actual_value: value.scalar_type(),
            expected_count,
            actual_count: count.scalar_type(),
        });
    }
    Ok(())
}

fn require_same_integer_type(
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Result<(), PropositionError> {
    require_same_type(left, right)?;
    if !matches!(left.scalar_type(), ScalarType::Integer(_)) {
        return Err(PropositionError::OrderedComparisonRequiresIntegers(
            left.scalar_type(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PropositionError {
    InvalidIntegerWidth(u16),
    IntegerLiteralOutsideType {
        scalar_type: IntegerType,
        value: IntegerValue,
    },
    MismatchedScalarTypes {
        left: ScalarType,
        right: ScalarType,
    },
    BooleanNotTypeMismatch(ScalarType),
    BooleanEqualTypeMismatch {
        left: ScalarType,
        right: ScalarType,
    },
    IntegerEqualTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerBitwiseNotTypeMismatch {
        expected: ScalarType,
        operand: ScalarType,
    },
    IntegerWidenTypeMismatch {
        source: ScalarType,
        target: ScalarType,
        operand: ScalarType,
    },
    IntegerExactCastTypeMismatch {
        source: ScalarType,
        target: ScalarType,
        operand: ScalarType,
    },
    IntegerOperandTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    IntegerShiftOperandTypeMismatch {
        expected_value: ScalarType,
        actual_value: ScalarType,
        expected_count: ScalarType,
        actual_count: ScalarType,
    },
    WrappingIntegerAddTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    SaturatingIntegerAddTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerSubtractTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    SaturatingIntegerSubtractTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    WrappingIntegerMultiplyTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    SaturatingIntegerMultiplyTypeMismatch {
        expected: ScalarType,
        left: ScalarType,
        right: ScalarType,
    },
    OrderedComparisonRequiresIntegers(ScalarType),
    NonCanonicalConjunctionArity(usize),
    NonCanonicalDisjunctionArity(usize),
    EmptyIeeeFloatStructuralFieldPath,
    NonCanonicalIeeeFloatComparisonOperands,
    EmptyByteSequenceStructuralFieldPath,
    NonCanonicalByteSequenceEqualOperands,
    EmptyContentAlgebraParameter,
    EmptyContentCaseName,
    EmptyContentFieldName,
    ZeroContentProjectionFingerprint,
    NonCanonicalContentEquationOrder,
    NonCanonicalContentSeparationArity(usize),
    NonCanonicalContentSeparationOrder,
    NestedContentSeparation,
    ContentTermNestingTooDeep,
    UnknownValue(ValueId),
    UnknownStructuralPlace(PlaceId),
    EntryResultStructuralPlace(PlaceId),
    UnsupportedContentLocalStructuralPlace(PlaceId),
    ConflictingValueType {
        id: ValueId,
        first: ScalarType,
        second: ScalarType,
    },
    ValueTypeMismatch {
        id: ValueId,
        expected: ScalarType,
        actual: ScalarType,
    },
    ConflictingStructuralPlaceKind {
        id: PlaceId,
        first: StructuralPlaceKind,
        second: StructuralPlaceKind,
    },
}

impl std::fmt::Display for PropositionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{self:?}")
    }
}

impl std::error::Error for PropositionError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn integer_widening_requires_range_containment_and_preserves_closed_values() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i64_type = IntegerType::new(IntegerSign::Signed, 64).expect("i64");

        assert!(u8_type.can_widen_to(u16_type));
        assert!(i8_type.can_widen_to(i64_type));
        assert!(u8_type.can_widen_to(i64_type));
        assert!(!u16_type.can_widen_to(u8_type));
        assert!(!u8_type.can_widen_to(u8_type));
        assert!(!i8_type.can_widen_to(u16_type));

        let widened = ScalarTerm::integer_widen(
            i8_type,
            i64_type,
            ScalarTerm::integer(i8_type, IntegerValue::Signed(-128)).expect("i8 literal"),
        )
        .expect("signed widening");
        assert_eq!(widened.scalar_type(), ScalarType::Integer(i64_type));
        assert_eq!(
            widened.integer_value(),
            Some((i64_type, IntegerValue::Signed(-128)))
        );

        let cross_signedness = ScalarTerm::integer_widen(
            u8_type,
            i64_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).expect("u8 literal"),
        )
        .expect("the complete u8 range fits i64");
        assert_eq!(
            cross_signedness.integer_value(),
            Some((i64_type, IntegerValue::Signed(255)))
        );

        let narrowing = ScalarTerm::integer_widen(
            u16_type,
            u8_type,
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(1)).expect("u16 literal"),
        );
        assert!(matches!(
            narrowing,
            Err(PropositionError::IntegerWidenTypeMismatch { .. })
        ));
    }

    #[test]
    fn integer_literals_are_checked_against_their_terminal_type() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
        assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).is_ok());
        assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(128)).is_err());

        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
        assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).is_ok());
        assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(256)).is_err());
    }

    #[test]
    fn address_carriers_remain_distinct_from_same_width_unsigned_integers() {
        let address = IntegerType::address(64).expect("addr");
        let u64_type = IntegerType::new(IntegerSign::Unsigned, 64).expect("u64");

        assert_eq!(address.carrier(), IntegerCarrier::Address);
        assert!(address.is_address());
        assert_eq!(address.sign(), IntegerSign::Unsigned);
        assert_eq!(address.bits(), 64);
        assert_ne!(address, u64_type);
        assert!(!address.can_widen_to(u64_type));
        assert!(!u64_type.can_widen_to(address));
        assert!(address.admits(IntegerValue::Unsigned(u64::MAX.into())));
    }

    #[test]
    fn ordered_comparisons_require_one_exact_integer_type() {
        let boolean = ScalarTerm::boolean(false);
        assert_eq!(
            Proposition::LessThan(boolean.clone(), boolean)
                .validate()
                .expect_err("booleans are unordered"),
            PropositionError::OrderedComparisonRequiresIntegers(ScalarType::Boolean)
        );
    }

    #[test]
    fn boolean_not_is_typed_and_reduces_closed_terms() {
        let negated = ScalarTerm::boolean_not(ScalarTerm::boolean(false)).unwrap();
        assert_eq!(negated.scalar_type(), ScalarType::Boolean);
        assert_eq!(negated.boolean_value(), Some(true));
        assert_eq!(
            ScalarTerm::boolean_not(
                ScalarTerm::integer(
                    IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
                    IntegerValue::Unsigned(1),
                )
                .unwrap(),
            ),
            Err(PropositionError::BooleanNotTypeMismatch(
                ScalarType::Integer(IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"))
            ))
        );
    }

    #[test]
    fn boolean_equality_is_typed_and_reduces_closed_terms() {
        let equal =
            ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(false))
                .unwrap();
        let unequal =
            ScalarTerm::boolean_equal(ScalarTerm::boolean(false), ScalarTerm::boolean(true))
                .unwrap();
        assert_eq!(equal.scalar_type(), ScalarType::Boolean);
        assert_eq!(equal.boolean_value(), Some(true));
        assert_eq!(unequal.boolean_value(), Some(false));

        let integer = ScalarTerm::integer(
            IntegerType::new(IntegerSign::Unsigned, 8).expect("u8"),
            IntegerValue::Unsigned(1),
        )
        .unwrap();
        assert!(matches!(
            ScalarTerm::boolean_equal(ScalarTerm::boolean(true), integer),
            Err(PropositionError::BooleanEqualTypeMismatch { .. })
        ));
    }

    #[test]
    fn integer_equality_is_typed_and_reduces_closed_terms() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let integer = |value| {
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value))
                .expect("u8 literal is representable")
        };
        let equal = ScalarTerm::integer_equal(u8_type, integer(255), integer(255)).unwrap();
        let unequal = ScalarTerm::integer_equal(u8_type, integer(0), integer(255)).unwrap();
        assert_eq!(equal.scalar_type(), ScalarType::Boolean);
        assert_eq!(equal.boolean_value(), Some(true));
        assert_eq!(unequal.boolean_value(), Some(false));

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed = ScalarTerm::integer(i8_type, IntegerValue::Signed(-1)).unwrap();
        assert!(matches!(
            ScalarTerm::integer_equal(u8_type, integer(255), signed),
            Err(PropositionError::IntegerEqualTypeMismatch { .. })
        ));
    }

    #[test]
    fn integer_ordering_is_typed_and_respects_signedness() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned = |value| {
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value)).expect("u8 literal")
        };
        let less = ScalarTerm::integer_less_than(u8_type, unsigned(1), unsigned(255)).unwrap();
        let less_or_equal =
            ScalarTerm::integer_less_or_equal(u8_type, unsigned(255), unsigned(255)).unwrap();
        assert_eq!(less.scalar_type(), ScalarType::Boolean);
        assert_eq!(less.boolean_value(), Some(true));
        assert_eq!(less_or_equal.boolean_value(), Some(true));

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed =
            |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
        assert_eq!(
            ScalarTerm::integer_less_than(i8_type, signed(-1), signed(0))
                .unwrap()
                .boolean_value(),
            Some(true)
        );
        assert!(matches!(
            ScalarTerm::integer_less_than(u8_type, unsigned(1), signed(-1)),
            Err(PropositionError::IntegerOperandTypeMismatch { .. })
        ));
    }

    #[test]
    fn integer_bitwise_operations_are_typed_and_reduce_closed_terms() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let unsigned = |value| {
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(value)).expect("u8 literal")
        };
        let and = ScalarTerm::integer_bitwise_and(u8_type, unsigned(0b1100), unsigned(0b1010))
            .expect("matching integer operands");
        let or = ScalarTerm::integer_bitwise_or(u8_type, unsigned(0b1100), unsigned(0b0011))
            .expect("matching integer operands");
        let xor = ScalarTerm::integer_bitwise_xor(u8_type, unsigned(0b1100), unsigned(0b1010))
            .expect("matching integer operands");
        let not = ScalarTerm::integer_bitwise_not(u8_type, unsigned(0b0000_1111))
            .expect("matching integer operand");
        assert_eq!(and.scalar_type(), ScalarType::Integer(u8_type));
        assert_eq!(
            and.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b1000)))
        );
        assert_eq!(
            or.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b1111)))
        );
        assert_eq!(
            xor.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b0110)))
        );
        assert_eq!(
            not.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(0b1111_0000)))
        );

        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let signed =
            |value| ScalarTerm::integer(i8_type, IntegerValue::Signed(value)).expect("i8 literal");
        assert_eq!(
            ScalarTerm::integer_bitwise_xor(i8_type, signed(-1), signed(-128))
                .unwrap()
                .integer_value(),
            Some((i8_type, IntegerValue::Signed(127)))
        );
        assert_eq!(
            ScalarTerm::integer_bitwise_not(i8_type, signed(-128))
                .unwrap()
                .integer_value(),
            Some((i8_type, IntegerValue::Signed(127)))
        );
        assert!(matches!(
            ScalarTerm::integer_bitwise_not(u8_type, signed(1)),
            Err(PropositionError::IntegerBitwiseNotTypeMismatch { .. })
        ));
        assert!(matches!(
            ScalarTerm::integer_bitwise_and(u8_type, unsigned(1), signed(1)),
            Err(PropositionError::IntegerOperandTypeMismatch { .. })
        ));
    }

    #[test]
    fn wrapping_shifts_reduce_counts_modulo_the_value_width() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let u16_type = IntegerType::new(IntegerSign::Unsigned, 16).expect("u16");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        assert_eq!(
            u8_type.wrapping_shift_left(
                IntegerValue::Unsigned(1),
                u16_type,
                IntegerValue::Unsigned(9),
            ),
            Some(IntegerValue::Unsigned(2))
        );
        assert_eq!(
            i8_type.wrapping_shift_right(
                IntegerValue::Signed(-8),
                u16_type,
                IntegerValue::Unsigned(9),
            ),
            Some(IntegerValue::Signed(-4))
        );
        assert_eq!(
            u8_type.wrapping_shift_left(
                IntegerValue::Unsigned(1),
                i8_type,
                IntegerValue::Signed(-1),
            ),
            Some(IntegerValue::Unsigned(128))
        );

        // Terminal Psi admits exact widths that are not native source widths;
        // modulo is the semantic rule, rather than a power-of-two bit mask.
        let u6_type = IntegerType::new(IntegerSign::Unsigned, 6).expect("u6");
        assert_eq!(
            u6_type.wrapping_shift_left(
                IntegerValue::Unsigned(1),
                u16_type,
                IntegerValue::Unsigned(7),
            ),
            Some(IntegerValue::Unsigned(2))
        );

        let term = ScalarTerm::wrapping_integer_shift_left(
            u8_type,
            u16_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).unwrap(),
            ScalarTerm::integer(u16_type, IntegerValue::Unsigned(9)).unwrap(),
        )
        .expect("independently typed count");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(2)))
        );
    }

    #[test]
    fn exact_right_shifts_require_an_in_range_nonnegative_count() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");

        assert_eq!(
            u8_type.exact_shift_right(
                IntegerValue::Unsigned(0b1000_0000),
                i16_type,
                IntegerValue::Signed(7),
            ),
            Some(IntegerValue::Unsigned(1))
        );
        assert_eq!(
            i8_type.exact_shift_right(IntegerValue::Signed(-8), i16_type, IntegerValue::Signed(2),),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            u8_type.exact_shift_right(
                IntegerValue::Unsigned(1),
                i16_type,
                IntegerValue::Signed(-1),
            ),
            None
        );
        assert_eq!(
            u8_type
                .exact_shift_right(IntegerValue::Unsigned(1), i16_type, IntegerValue::Signed(8),),
            None
        );

        let term = ScalarTerm::exact_integer_shift_right(
            u8_type,
            i16_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(128)).unwrap(),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(7)).unwrap(),
        )
        .expect("exact right shift with an independently typed count");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(1)))
        );
    }

    #[test]
    fn exact_left_shifts_require_a_legal_count_and_representable_result() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8");
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8");
        let i16_type = IntegerType::new(IntegerSign::Signed, 16).expect("i16");

        assert_eq!(
            u8_type.exact_shift_left(IntegerValue::Unsigned(1), i16_type, IntegerValue::Signed(7),),
            Some(IntegerValue::Unsigned(128))
        );
        assert_eq!(
            u8_type.exact_shift_left(IntegerValue::Unsigned(2), i16_type, IntegerValue::Signed(7),),
            None
        );
        assert_eq!(
            i8_type.exact_shift_left(IntegerValue::Signed(-1), i16_type, IntegerValue::Signed(7),),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.exact_shift_left(IntegerValue::Signed(1), i16_type, IntegerValue::Signed(-1),),
            None
        );

        let term = ScalarTerm::exact_integer_shift_left(
            u8_type,
            i16_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(1)).unwrap(),
            ScalarTerm::integer(i16_type, IntegerValue::Signed(7)).unwrap(),
        )
        .expect("exact left shift with an independently typed count");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(128)))
        );
    }

    #[test]
    fn proposition_context_rejects_value_type_reinterpretation() {
        let id = ValueId::new(7).expect("value identity");
        let i32_type = IntegerType::new(IntegerSign::Signed, 32).expect("i32 type");
        let context = PropositionContext::from_value_types([(id, ScalarType::Integer(i32_type))])
            .expect("value context");
        let proposition = Proposition::Equal(
            ScalarTerm::value(id, ScalarType::Boolean),
            ScalarTerm::boolean(true),
        );
        assert!(matches!(
            context.validate(&proposition),
            Err(PropositionError::ValueTypeMismatch { .. })
        ));
    }

    #[test]
    fn wrapping_add_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
            Some(IntegerValue::Unsigned(44))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(-116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(0))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn exact_add_rejects_sums_outside_the_declared_carrier() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(55)),
            Some(IntegerValue::Unsigned(255))
        );
        assert_eq!(
            u8_type.exact_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(56)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_add(IntegerValue::Signed(120), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.exact_add(IntegerValue::Signed(-120), IntegerValue::Signed(-9)),
            None
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.exact_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            None
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.exact_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            None
        );
    }

    #[test]
    fn exact_sub_rejects_differences_outside_the_declared_carrier() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.exact_sub(IntegerValue::Unsigned(4), IntegerValue::Unsigned(5)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(-120), IntegerValue::Signed(8)),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(-121), IntegerValue::Signed(8)),
            None
        );
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(120), IntegerValue::Signed(-7)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.exact_sub(IntegerValue::Signed(121), IntegerValue::Signed(-7)),
            None
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.exact_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            None
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.exact_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            None
        );
    }

    #[test]
    fn exact_mul_rejects_products_outside_the_declared_carrier() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_mul(IntegerValue::Unsigned(51), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(255))
        );
        assert_eq!(
            u8_type.exact_mul(IntegerValue::Unsigned(52), IntegerValue::Unsigned(5)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_mul(IntegerValue::Signed(-42), IntegerValue::Signed(3)),
            Some(IntegerValue::Signed(-126))
        );
        assert_eq!(
            i8_type.exact_mul(IntegerValue::Signed(-43), IntegerValue::Signed(3)),
            None
        );
        assert_eq!(
            i8_type.exact_mul(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            None
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.exact_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2)),
            None
        );
        let term = ScalarTerm::exact_integer_multiply(
            u8_type,
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(51)).unwrap(),
            ScalarTerm::integer(u8_type, IntegerValue::Unsigned(5)).unwrap(),
        )
        .expect("exact multiply term");
        assert_eq!(
            term.integer_value(),
            Some((u8_type, IntegerValue::Unsigned(255)))
        );
    }

    #[test]
    fn exact_div_rejects_zero_and_signed_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(51))
        );
        assert_eq!(
            u8_type.exact_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.exact_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            None
        );
    }

    #[test]
    fn exact_rem_is_truncating_and_rejects_undefined_quotients() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.exact_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.exact_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.exact_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            i8_type.exact_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            None
        );
    }

    #[test]
    fn wrapping_div_reduces_the_signed_minimum_quotient_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(51))
        );
        assert_eq!(
            u8_type.wrapping_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.wrapping_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(-128))
        );
    }

    #[test]
    fn wrapping_rem_reduces_the_signed_minimum_quotient_overflow_to_zero() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.wrapping_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            i8_type.wrapping_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(0))
        );
    }

    #[test]
    fn saturating_div_clamps_the_signed_minimum_quotient_overflow() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(51))
        );
        assert_eq!(
            u8_type.saturating_div(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_div(IntegerValue::Signed(-127), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_div(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(127))
        );
    }

    #[test]
    fn saturating_rem_reduces_the_signed_minimum_quotient_overflow_to_zero() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(5)),
            Some(IntegerValue::Unsigned(0))
        );
        assert_eq!(
            u8_type.saturating_rem(IntegerValue::Unsigned(255), IntegerValue::Unsigned(0)),
            None
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_rem(IntegerValue::Signed(-127), IntegerValue::Signed(5)),
            Some(IntegerValue::Signed(-2))
        );
        assert_eq!(
            i8_type.saturating_rem(IntegerValue::Signed(-128), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(0))
        );
    }

    #[test]
    fn saturating_add_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_add(IntegerValue::Unsigned(200), IntegerValue::Unsigned(100)),
            Some(IntegerValue::Unsigned(255))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_add(IntegerValue::Signed(120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_add(IntegerValue::Signed(-120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(-128))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_add(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_add(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
        assert_eq!(
            i128_type.saturating_add(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn wrapping_subtract_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
            Some(IntegerValue::Unsigned(251))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(116))
        );
        assert_eq!(
            i8_type.wrapping_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(-116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }

    #[test]
    fn saturating_subtract_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_sub(IntegerValue::Unsigned(5), IntegerValue::Unsigned(10)),
            Some(IntegerValue::Unsigned(0))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_sub(IntegerValue::Signed(-120), IntegerValue::Signed(20)),
            Some(IntegerValue::Signed(-128))
        );
        assert_eq!(
            i8_type.saturating_sub(IntegerValue::Signed(120), IntegerValue::Signed(-20)),
            Some(IntegerValue::Signed(127))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_sub(IntegerValue::Unsigned(0), IntegerValue::Unsigned(1)),
            Some(IntegerValue::Unsigned(0))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_sub(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
        assert_eq!(
            i128_type.saturating_sub(IntegerValue::Signed(i128::MAX), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }

    #[test]
    fn wrapping_multiply_reduces_at_the_declared_width_for_all_edge_shapes() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.wrapping_mul(IntegerValue::Unsigned(20), IntegerValue::Unsigned(13)),
            Some(IntegerValue::Unsigned(4))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.wrapping_mul(IntegerValue::Signed(20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(-116))
        );
        assert_eq!(
            i8_type.wrapping_mul(IntegerValue::Signed(-20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(116))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.wrapping_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2)),
            Some(IntegerValue::Unsigned(u128::MAX - 1))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.wrapping_mul(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1)),
            Some(IntegerValue::Signed(i128::MIN))
        );
    }

    #[test]
    fn saturating_multiply_clamps_at_declared_signed_and_unsigned_bounds() {
        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).unwrap();
        assert_eq!(
            u8_type.saturating_mul(IntegerValue::Unsigned(20), IntegerValue::Unsigned(13)),
            Some(IntegerValue::Unsigned(255))
        );
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).unwrap();
        assert_eq!(
            i8_type.saturating_mul(IntegerValue::Signed(20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(127))
        );
        assert_eq!(
            i8_type.saturating_mul(IntegerValue::Signed(-20), IntegerValue::Signed(7)),
            Some(IntegerValue::Signed(-128))
        );
        let u128_type = IntegerType::new(IntegerSign::Unsigned, 128).unwrap();
        assert_eq!(
            u128_type.saturating_mul(IntegerValue::Unsigned(u128::MAX), IntegerValue::Unsigned(2),),
            Some(IntegerValue::Unsigned(u128::MAX))
        );
        let i128_type = IntegerType::new(IntegerSign::Signed, 128).unwrap();
        assert_eq!(
            i128_type.saturating_mul(IntegerValue::Signed(i128::MIN), IntegerValue::Signed(-1),),
            Some(IntegerValue::Signed(i128::MAX))
        );
    }
}
