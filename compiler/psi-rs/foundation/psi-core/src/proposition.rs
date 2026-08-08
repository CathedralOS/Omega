use std::{cmp::Ordering, collections::BTreeMap};

use crate::{
    ContentConservation, ContentPlaceVersion, ContentTerm, PlaceId, PropositionId,
    StructuralPlaceKind, ValueId,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum IntegerSign {
    Signed,
    Unsigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IntegerType {
    sign: IntegerSign,
    bits: u16,
}

impl IntegerType {
    pub fn new(sign: IntegerSign, bits: u16) -> Result<Self, PropositionError> {
        if !(1..=128).contains(&bits) {
            return Err(PropositionError::InvalidIntegerWidth(bits));
        }
        Ok(Self { sign, bits })
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ScalarTerm {
    Value {
        id: ValueId,
        scalar_type: ScalarType,
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
            Self::Boolean(_)
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
            | Self::WrappingIntegerAdd { scalar_type, .. }
            | Self::SaturatingIntegerAdd { scalar_type, .. }
            | Self::WrappingIntegerSubtract { scalar_type, .. }
            | Self::SaturatingIntegerSubtract { scalar_type, .. }
            | Self::WrappingIntegerMultiply { scalar_type, .. }
            | Self::SaturatingIntegerMultiply { scalar_type, .. } => {
                ScalarType::Integer(*scalar_type)
            }
            Self::WrappingIntegerShiftLeft { value_type, .. }
            | Self::WrappingIntegerShiftRight { value_type, .. } => {
                ScalarType::Integer(*value_type)
            }
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
                    _ => unreachable!(),
                };
                Some((*value_type, result))
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
            Self::Value { .. } | Self::Boolean(_) => Ok(()),
            Self::BooleanNot { operand } => {
                operand.validate()?;
                if operand.scalar_type() != ScalarType::Boolean {
                    return Err(PropositionError::BooleanNotTypeMismatch(
                        operand.scalar_type(),
                    ));
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
    Conjunction(Vec<Proposition>),
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
            Proposition::Conjunction(conjuncts) => {
                for conjunct in conjuncts {
                    self.validate_value_terms(conjunct)?;
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
                    && *kind == StructuralPlaceKind::Result
                {
                    return Err(PropositionError::EntryResultStructuralPlace(subject.root));
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
            ScalarTerm::WrappingIntegerAdd { left, right, .. }
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
            | ScalarTerm::WrappingIntegerShiftRight { value, count, .. } => {
                self.validate_term(value)?;
                self.validate_term(count)?;
            }
            ScalarTerm::BooleanNot { operand } | ScalarTerm::IntegerBitwiseNot { operand, .. } => {
                self.validate_term(operand)?
            }
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
    fn integer_literals_are_checked_against_their_terminal_type() {
        let i8_type = IntegerType::new(IntegerSign::Signed, 8).expect("i8 type");
        assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(127)).is_ok());
        assert!(ScalarTerm::integer(i8_type, IntegerValue::Signed(128)).is_err());

        let u8_type = IntegerType::new(IntegerSign::Unsigned, 8).expect("u8 type");
        assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(255)).is_ok());
        assert!(ScalarTerm::integer(u8_type, IntegerValue::Unsigned(256)).is_err());
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
