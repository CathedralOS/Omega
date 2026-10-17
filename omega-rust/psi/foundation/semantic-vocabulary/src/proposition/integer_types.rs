//! Integer types and their closed arithmetic: sign, carrier and width, the
//! values and math literals they admit, and the math terms built over them.

use crate::ValueId;
use crate::proposition::{PropositionError, ScalarTerm, ScalarType};
use std::cmp::Ordering;
mod bit_ranges;

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

macro_rules! wrapping_arith {
    ($( $(#[$m:meta])* $name:ident : $op:ident ; )+) => {$(
        $(#[$m])*
        pub fn $name(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
            self.admits_both(left, right)?;
            let mask = if self.bits == 128 {
                u128::MAX
            } else {
                (1_u128 << self.bits) - 1
            };
            self.dispatch_sign(
                left,
                right,
                |left, right| Some(IntegerValue::Unsigned(left.$op(right) & mask)),
                |left, right| {
                    let bits = (left as u128).$op(right as u128) & mask;
                    let value = if self.bits == 128 || bits & (1_u128 << (self.bits - 1)) == 0 {
                        bits as i128
                    } else {
                        (bits | !mask) as i128
                    };
                    Some(IntegerValue::Signed(value))
                },
            )
        }
    )+}
}

impl IntegerType {
    /// Exact subtraction preserves address/count roles. Address offsets consume
    /// u64 counts; two addresses produce a u64 distance. Width equality alone
    /// grants neither conversion nor address arithmetic authority.
    pub fn admits_exact_subtract_operands(self, left: Self, right: Self) -> bool {
        let count = Self::new(IntegerSign::Unsigned, 64).expect("u64 count");
        if left.is_address() {
            (self == left && right == count) || (self == count && right == left)
        } else {
            !self.is_address() && left == self && right == self
        }
    }

    /// Operands retain their own admission ranges, including addresses wider
    /// than the count carrier. Only the mathematical difference must fit here.
    pub fn exact_subtract_values(
        self,
        left_type: Self,
        left: IntegerValue,
        right_type: Self,
        right: IntegerValue,
    ) -> Option<IntegerValue> {
        if !self.admits_exact_subtract_operands(left_type, right_type)
            || !left_type.admits(left)
            || !right_type.admits(right)
        {
            return None;
        }
        let result = match (left, right) {
            (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => {
                IntegerValue::Unsigned(left.checked_sub(right)?)
            }
            (IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                IntegerValue::Signed(left.checked_sub(right)?)
            }
            _ => return None,
        };
        self.admits(result).then_some(result)
    }

    pub fn can_exact_cast_to(self, target: Self) -> bool {
        // This selects a denotation, not a total conversion. Its operation
        // still proves representability in the exact destination carrier.
        // Address model changes are not count interpretation and remain out.
        self.carrier == IntegerCarrier::Fixed || target.carrier == IntegerCarrier::Fixed
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

    /// The additive identity, carried in the type's own signedness.
    pub const fn zero(self) -> IntegerValue {
        match self.sign {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        }
    }

    /// The multiplicative identity, carried in the type's own signedness.
    pub const fn one(self) -> IntegerValue {
        match self.sign {
            IntegerSign::Signed => IntegerValue::Signed(1),
            IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        }
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
    /// Both operands admitted, or `None` for the caller's early-out.
    fn admits_both(self, left: IntegerValue, right: IntegerValue) -> Option<()> {
        (self.admits(left) && self.admits(right)).then_some(())
    }

    /// Compare two values under this exact integer type's signedness.
    ///
    /// Values of the wrong sign or outside the declared width reject rather
    /// than being reinterpreted.
    pub fn compare(self, left: IntegerValue, right: IntegerValue) -> Option<Ordering> {
        self.compare_with(left, self, right)
    }

    /// Compare mathematical values while preserving each operand's carrier.
    /// Payloads must inhabit their own exact type; address carriers do not
    /// acquire mixed fixed-integer comparison meaning.
    pub fn compare_with(
        self,
        left: IntegerValue,
        right_type: Self,
        right: IntegerValue,
    ) -> Option<Ordering> {
        if !self.admits(left)
            || !right_type.admits(right)
            || (self != right_type && (self.is_address() || right_type.is_address()))
        {
            return None;
        }
        Some(match (left, right) {
            (IntegerValue::Unsigned(left), IntegerValue::Unsigned(right)) => left.cmp(&right),
            (IntegerValue::Signed(left), IntegerValue::Signed(right)) => left.cmp(&right),
            (IntegerValue::Signed(left), IntegerValue::Unsigned(right)) => {
                if left < 0 {
                    Ordering::Less
                } else {
                    (left as u128).cmp(&right)
                }
            }
            (IntegerValue::Unsigned(left), IntegerValue::Signed(right)) => {
                if right < 0 {
                    Ordering::Greater
                } else {
                    left.cmp(&(right as u128))
                }
            }
        })
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
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(unsigned(left, right))),
            |left, right| Some(IntegerValue::Signed(signed(left, right))),
        )
    }

    fn dispatch_sign(
        self,
        left: IntegerValue,
        right: IntegerValue,
        unsigned: impl FnOnce(u128, u128) -> Option<IntegerValue>,
        signed: impl FnOnce(i128, i128) -> Option<IntegerValue>,
    ) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        match (self.sign, left, right) {
            (
                IntegerSign::Unsigned,
                IntegerValue::Unsigned(left),
                IntegerValue::Unsigned(right),
            ) => unsigned(left, right),
            (IntegerSign::Signed, IntegerValue::Signed(left), IntegerValue::Signed(right)) => {
                signed(left, right)
            }
            _ => None,
        }
    }

    fn signed_minimum(self) -> i128 {
        let IntegerValue::Signed(minimum) = self.minimum_value() else {
            unreachable!("signed type has a signed minimum")
        };
        minimum
    }

    /// The signed arm every truncating quotient/remainder shares: the
    /// minimum/-1 operand pair is the sole overflow and yields `on_overflow`;
    /// every other pair runs the checked op and re-admits the result.
    fn signed_truncating_or(
        self,
        left: i128,
        right: i128,

        op: impl Fn(i128, i128) -> Option<i128>,
        on_overflow: IntegerValue,
    ) -> Option<IntegerValue> {
        let minimum = self.signed_minimum();
        if left == minimum && right == -1 {
            Some(on_overflow)
        } else {
            let result = IntegerValue::Signed(op(left, right)?);
            self.admits(result).then_some(result)
        }
    }

    wrapping_arith! {
        /// Add two admitted values modulo this exact integer width.
        ///
        /// Signed results use two's-complement interpretation of the reduced bit
        /// pattern. A sign/value mismatch or out-of-range input is rejected rather
        /// than silently reinterpreted.
        wrapping_add : wrapping_add ;
    }

    /// Add two admitted values when their mathematical sum remains admitted.
    pub fn exact_add(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        let result = self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_add(right)?)),
            |left, right| Some(IntegerValue::Signed(left.checked_add(right)?)),
        )?;
        self.admits(result).then_some(result)
    }

    /// Subtract two admitted values when their mathematical difference remains admitted.
    pub fn exact_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        let result = self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_sub(right)?)),
            |left, right| Some(IntegerValue::Signed(left.checked_sub(right)?)),
        )?;
        self.admits(result).then_some(result)
    }

    /// Multiply two admitted values when their mathematical product remains admitted.
    pub fn exact_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        let result = self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_mul(right)?)),
            |left, right| Some(IntegerValue::Signed(left.checked_mul(right)?)),
        )?;
        self.admits(result).then_some(result)
    }

    /// Divide two admitted values with truncation toward zero when the divisor
    /// is nonzero and the mathematical quotient remains admitted.
    pub fn exact_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        let result = self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_div(right)?)),
            |left, right| Some(IntegerValue::Signed(left.checked_div(right)?)),
        )?;
        self.admits(result).then_some(result)
    }

    /// Compute a truncating remainder when the corresponding quotient is
    /// defined and the result remains admitted.
    pub fn exact_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        let result = self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_rem(right)?)),
            |left, right| {
                Some({
                    let minimum = self.signed_minimum();
                    if left == minimum && right == -1 {
                        return None;
                    }
                    IntegerValue::Signed(left.checked_rem(right)?)
                })
            },
        )?;
        self.admits(result).then_some(result)
    }

    /// Divide two admitted values with truncation toward zero and reduce the
    /// sole signed quotient overflow modulo this integer width.
    pub fn wrapping_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_div(right)?)),
            |left, right| {
                self.signed_truncating_or(
                    left,
                    right,
                    |left, right| left.checked_div(right),
                    IntegerValue::Signed(self.signed_minimum()),
                )
            },
        )
    }

    /// Compute a truncating remainder and reduce the sole signed quotient
    /// overflow to the wrapping-policy result of zero.
    pub fn wrapping_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_rem(right)?)),
            |left, right| {
                self.signed_truncating_or(
                    left,
                    right,
                    |left, right| left.checked_rem(right),
                    IntegerValue::Signed(0),
                )
            },
        )
    }

    /// Divide two admitted values with truncation toward zero and clamp the
    /// sole signed quotient overflow to this integer width's maximum.
    pub fn saturating_div(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.checked_div(right)?)),
            |left, right| {
                self.signed_truncating_or(
                    left,
                    right,
                    |left, right| left.checked_div(right),
                    self.maximum_value(),
                )
            },
        )
    }

    /// Compute a truncating remainder and reduce the sole signed quotient
    /// overflow to the saturating-policy result of zero.
    pub fn saturating_rem(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.wrapping_rem(left, right)
    }

    wrapping_arith! {
        /// Subtract two admitted values modulo this exact integer width.
        ///
        /// Signed results use two's-complement interpretation of the reduced bit
        /// pattern. A sign/value mismatch or out-of-range input is rejected rather
        /// than silently reinterpreted.
        wrapping_sub : wrapping_sub ;
    }

    wrapping_arith! {
        /// Multiply two admitted values modulo this exact integer width.
        ///
        /// Signed results use two's-complement interpretation of the reduced bit
        /// pattern. A sign/value mismatch or out-of-range input is rejected rather
        /// than silently reinterpreted.
        wrapping_mul : wrapping_mul ;
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

    /// Bound exact right shifts over ordered value and independently typed
    /// count ranges. Every endpoint must form under the exact value kernel.
    /// Right shift is monotone in the value and monotone in the count within
    /// each value sign region, so the four corners contain all results even
    /// when the value range crosses zero. Invalid counts supply no bound.
    pub fn exact_shift_right_bounds(
        self,
        value: (IntegerValue, IntegerValue),
        count_type: IntegerType,
        count: (IntegerValue, IntegerValue),
    ) -> Option<(IntegerValue, IntegerValue)> {
        self.exact_shift_bounds(value, count_type, count, Self::exact_shift_right)
    }

    /// Bound exact left shifts without losing unsigned ceilings or treating
    /// the count as a value-carrier operand. All four corners must form under
    /// the exact kernel: a valid count alone does not prove representability.
    pub fn exact_shift_left_bounds(
        self,
        value: (IntegerValue, IntegerValue),
        count_type: IntegerType,
        count: (IntegerValue, IntegerValue),
    ) -> Option<(IntegerValue, IntegerValue)> {
        self.exact_shift_bounds(value, count_type, count, Self::exact_shift_left)
    }

    // Both exact shifts are monotone in the value and monotone in the count
    // within each value sign region. Ordered, admitted endpoints therefore
    // bound all interior results, including ranges that cross zero. Keep this
    // hull shared, but let each exact value operation reject its own invalid
    // count and (for left shift) unrepresentable result before publishing it.
    fn exact_shift_bounds(
        self,
        value: (IntegerValue, IntegerValue),
        count_type: IntegerType,
        count: (IntegerValue, IntegerValue),
        shift: fn(Self, IntegerValue, Self, IntegerValue) -> Option<IntegerValue>,
    ) -> Option<(IntegerValue, IntegerValue)> {
        if self.compare(value.0, value.1)? == Ordering::Greater
            || count_type.compare(count.0, count.1)? == Ordering::Greater
        {
            return None;
        }
        let [Some(first), Some(second), Some(third), Some(fourth)] = [
            shift(self, value.0, count_type, count.0),
            shift(self, value.0, count_type, count.1),
            shift(self, value.1, count_type, count.0),
            shift(self, value.1, count_type, count.1),
        ] else {
            return None;
        };
        let (mut low, mut high) = (first, first);
        for corner in [second, third, fourth] {
            if self.compare(corner, low)? == Ordering::Less {
                low = corner;
            }
            if self.compare(corner, high)? == Ordering::Greater {
                high = corner;
            }
        }
        Some((low, high))
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
        bit_width_mask(self.bits)
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
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| {
                let maximum = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                Some(IntegerValue::Unsigned(
                    left.saturating_add(right).min(maximum),
                ))
            },
            |left, right| {
                let value = if self.bits == 128 {
                    left.saturating_add(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    (left + right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            },
        )
    }

    /// Subtract two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_sub(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| Some(IntegerValue::Unsigned(left.saturating_sub(right))),
            |left, right| {
                let value = if self.bits == 128 {
                    left.saturating_sub(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    (left - right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            },
        )
    }

    /// Multiply two admitted values and clamp the result to this exact integer
    /// type's representable bounds.
    ///
    /// A sign/value mismatch or out-of-range input is rejected rather than
    /// silently reinterpreted.
    pub fn saturating_mul(self, left: IntegerValue, right: IntegerValue) -> Option<IntegerValue> {
        self.admits_both(left, right)?;
        self.dispatch_sign(
            left,
            right,
            |left, right| {
                let maximum = if self.bits == 128 {
                    u128::MAX
                } else {
                    (1_u128 << self.bits) - 1
                };
                Some(IntegerValue::Unsigned(
                    left.saturating_mul(right).min(maximum),
                ))
            },
            |left, right| {
                let value = if self.bits == 128 {
                    left.saturating_mul(right)
                } else {
                    let limit = 1_i128 << (self.bits - 1);
                    left.saturating_mul(right).clamp(-limit, limit - 1)
                };
                Some(IntegerValue::Signed(value))
            },
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerValue {
    Signed(i128),
    Unsigned(u128),
}

impl IntegerValue {
    /// The value carried in `sign`'s representation. Unsigned carriers reject
    /// negative inputs through the checked conversion.
    pub fn for_sign(sign: IntegerSign, value: i128) -> Result<Self, std::num::TryFromIntError> {
        match sign {
            IntegerSign::Signed => Ok(Self::Signed(value)),
            IntegerSign::Unsigned => value.try_into().map(Self::Unsigned),
        }
    }
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
/// mathematical value of one exact integer or address carrier; the other forms are
/// interpreted over unbounded integers rather than machine arithmetic.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerMathTerm {
    IntegerLiteral(IntegerMathLiteral),
    MathValue {
        source_type: IntegerType,
        value: ValueId,
    },
    /// The mathematical value of one finite observation or expression. Its
    /// carrier and arithmetic policy remain inside the scalar term; embedding
    /// never reinterprets a wrapping operation as an unbounded operation.
    Embed(Box<ScalarTerm>),
    Add(Box<Self>, Box<Self>),
    Subtract(Box<Self>, Box<Self>),
    Multiply(Box<Self>, Box<Self>),
    ShiftLeft {
        value: Box<Self>,
        count: Box<Self>,
    },
}

impl IntegerMathTerm {
    /// The direct sub-term operands in authored order; leaves have none.
    pub fn operands(&self) -> Vec<&IntegerMathTerm> {
        match self {
            Self::MathValue { .. } | Self::IntegerLiteral(_) | Self::Embed(_) => Vec::new(),
            Self::Add(left, right) | Self::Subtract(left, right) | Self::Multiply(left, right) => {
                vec![left.as_ref(), right.as_ref()]
            }
            Self::ShiftLeft { value, count } => vec![value.as_ref(), count.as_ref()],
        }
    }

    /// [`Self::operands`] with mutable access.
    pub fn operands_mut(&mut self) -> Vec<&mut IntegerMathTerm> {
        match self {
            Self::MathValue { .. } | Self::IntegerLiteral(_) | Self::Embed(_) => Vec::new(),
            Self::Add(left, right) | Self::Subtract(left, right) | Self::Multiply(left, right) => {
                vec![left.as_mut(), right.as_mut()]
            }
            Self::ShiftLeft { value, count } => vec![value.as_mut(), count.as_mut()],
        }
    }

    pub fn literal(value: IntegerValue) -> Self {
        Self::IntegerLiteral(IntegerMathLiteral::from_integer_value(value))
    }

    /// Keep the canonical value/literal spelling when no compound observation
    /// is needed. Callers still validate the term in its actual namespace.
    pub fn embed(term: ScalarTerm) -> Option<Self> {
        term.scalar_type().as_integer()?;
        Some(match term {
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(source_type),
            } => Self::MathValue {
                source_type,
                value: id,
            },
            ScalarTerm::Integer { value, .. } => Self::literal(value),
            term => Self::Embed(Box::new(term)),
        })
    }

    pub fn validate(&self) -> Result<(), PropositionError> {
        // An explicit worklist keeps deep terms off the call stack: children
        // push in reverse so the left operand still validates first, matching
        // the error order the recursive validator produced.
        let mut pending = vec![self];
        while let Some(term) = pending.pop() {
            match term {
                // Mathematical embedding observes the exact retained carrier,
                // including its address model. It creates no fixed-integer
                // conversion or storage authority; context validation still
                // checks the source type against the named value.
                Self::MathValue { .. } => {}
                Self::Embed(term) => {
                    term.validate()?;
                    if term.scalar_type().as_integer().is_none() {
                        return Err(PropositionError::IntegerMathEmbeddingNotInteger);
                    }
                    if matches!(
                        term.as_ref(),
                        ScalarTerm::Value { .. } | ScalarTerm::Integer { .. }
                    ) {
                        return Err(PropositionError::NonCanonicalIntegerMathEmbedding);
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

/// The all-ones mask for `bits` significant bits (128 is the full `u128`).
pub(crate) fn bit_width_mask(bits: u16) -> u128 {
    if bits == 128 {
        u128::MAX
    } else {
        (1_u128 << bits) - 1
    }
}
