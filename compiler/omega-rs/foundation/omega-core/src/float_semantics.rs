//! Executable, host-independent floating-point meanings.
//!
//! Operations decode their landed operands to exact rational/special-value
//! meanings, perform exact arithmetic, and round once through the selected
//! format record. The interpreter, constant folder, proof layer, and target
//! validation can therefore share one definition instead of independently
//! inheriting the host process's floating-point behavior.

use std::cmp::Ordering;

use crate::bignum::{BigInt, BigRational, ExactFloat, IeeeRounding};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatSpecialValues {
    pub signed_zero: bool,
    pub subnormals: bool,
    pub infinity: bool,
    pub nan: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FloatFormat {
    pub radix: u32,
    pub precision: u32,
    pub minimum_normal_exponent: i32,
    pub maximum_normal_exponent: i32,
    pub minimum_subnormal_exponent: i32,
    pub specials: FloatSpecialValues,
    pub rounds_to_nearest_ties_to_even: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoundingDirection {
    NearestTiesToEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}

impl RoundingDirection {
    fn ieee(self) -> IeeeRounding {
        match self {
            Self::NearestTiesToEven => IeeeRounding::NearestTiesToEven,
            Self::TowardZero => IeeeRounding::TowardZero,
            Self::TowardPositive => IeeeRounding::TowardPositive,
            Self::TowardNegative => IeeeRounding::TowardNegative,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatClass {
    NaN,
    Infinity { negative: bool },
    Normal { negative: bool },
    Subnormal { negative: bool },
    Zero { negative: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IntegerFormat {
    pub bits: u32,
    pub signed: bool,
}

impl IntegerFormat {
    pub const U8: Self = Self::unsigned(8);
    pub const U16: Self = Self::unsigned(16);
    pub const U32: Self = Self::unsigned(32);
    pub const U64: Self = Self::unsigned(64);
    pub const I8: Self = Self::signed(8);
    pub const I16: Self = Self::signed(16);
    pub const I32: Self = Self::signed(32);
    pub const I64: Self = Self::signed(64);

    pub const fn unsigned(bits: u32) -> Self {
        assert!(bits > 0);
        Self {
            bits,
            signed: false,
        }
    }

    pub const fn signed(bits: u32) -> Self {
        assert!(bits > 0);
        Self { bits, signed: true }
    }

    fn bounds(self) -> (BigInt, BigInt) {
        if self.signed {
            let magnitude = BigInt::from_u64(1).shl_bits((self.bits - 1) as usize);
            (magnitude.negate(), magnitude.sub(&BigInt::from_u64(1)))
        } else {
            (
                BigInt::zero(),
                BigInt::from_u64(1)
                    .shl_bits(self.bits as usize)
                    .sub(&BigInt::from_u64(1)),
            )
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatToIntegerError {
    NonFinite,
    OutOfRange,
}

/// The non-finite result class rejected by the `Trapping` float-policy
/// adapter. The adapter decision depends only on the semantic result; callers
/// may use the operands and operation identity to produce a more specific
/// diagnostic without changing that decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FloatPolicyTrap {
    NaNResult,
    InfinityResult,
}

impl FloatFormat {
    pub const BINARY32: Self = Self {
        radix: 2,
        precision: 24,
        minimum_normal_exponent: -126,
        maximum_normal_exponent: 127,
        minimum_subnormal_exponent: -149,
        specials: FloatSpecialValues {
            signed_zero: true,
            subnormals: true,
            infinity: true,
            nan: true,
        },
        rounds_to_nearest_ties_to_even: true,
    };

    pub const BINARY64: Self = Self {
        radix: 2,
        precision: 53,
        minimum_normal_exponent: -1022,
        maximum_normal_exponent: 1023,
        minimum_subnormal_exponent: -1074,
        specials: FloatSpecialValues {
            signed_zero: true,
            subnormals: true,
            infinity: true,
            nan: true,
        },
        rounds_to_nearest_ties_to_even: true,
    };
}

/// The payload-erased proof view of one landed floating-point value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FloatMeaning {
    FiniteNonZero(BigRational),
    Zero { negative: bool },
    Infinity { negative: bool },
    NaN,
}

impl FloatMeaning {
    pub fn from_f32(value: f32) -> Self {
        Self::from_exact(ExactFloat::from_f32(value))
    }

    pub fn from_f64(value: f64) -> Self {
        Self::from_exact(ExactFloat::from_f64(value))
    }

    pub fn to_f32(&self) -> f32 {
        self.to_exact().to_f32()
    }

    pub fn to_f64(&self) -> f64 {
        self.to_exact().to_f64()
    }

    pub fn to_interpreter_value(&self, format: FloatFormat) -> f64 {
        if format == FloatFormat::BINARY32 {
            f64::from(self.to_f32())
        } else if format == FloatFormat::BINARY64 {
            self.to_f64()
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    pub fn is_finite(&self) -> bool {
        matches!(self, Self::FiniteNonZero(_) | Self::Zero { .. })
    }

    pub fn is_nan(&self) -> bool {
        matches!(self, Self::NaN)
    }

    pub fn is_infinite(&self) -> bool {
        matches!(self, Self::Infinity { .. })
    }

    pub fn is_zero(&self) -> bool {
        matches!(self, Self::Zero { .. })
    }

    pub fn is_negative(&self) -> bool {
        match self {
            Self::FiniteNonZero(value) => value.is_negative(),
            Self::Zero { negative } | Self::Infinity { negative } => *negative,
            Self::NaN => false,
        }
    }

    fn from_exact(value: ExactFloat) -> Self {
        match value {
            ExactFloat::Finite(value) if value.is_zero() => Self::Zero {
                negative: value.is_negative(),
            },
            ExactFloat::Finite(value) => Self::FiniteNonZero(value),
            ExactFloat::Infinity { negative } => Self::Infinity { negative },
            ExactFloat::NaN => Self::NaN,
        }
    }

    fn to_exact(&self) -> ExactFloat {
        match self {
            Self::FiniteNonZero(value) => ExactFloat::Finite(value.clone()),
            Self::Zero { negative } => {
                ExactFloat::from_decimal_str(if *negative { "-0.0" } else { "0.0" })
                    .expect("zero spelling is exact")
            }
            Self::Infinity { negative } => ExactFloat::Infinity {
                negative: *negative,
            },
            Self::NaN => ExactFloat::NaN,
        }
    }
}

pub struct FloatSemantics;

impl FloatSemantics {
    pub fn from_decimal(format: FloatFormat, text: &str) -> Option<FloatMeaning> {
        Some(Self::round_exact(
            format,
            ExactFloat::from_decimal_str(text)?,
        ))
    }

    pub fn round_exact(format: FloatFormat, value: ExactFloat) -> FloatMeaning {
        Self::round_exact_directed(format, value, RoundingDirection::NearestTiesToEven)
    }

    pub fn round_exact_directed(
        format: FloatFormat,
        value: ExactFloat,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        if format == FloatFormat::BINARY32 {
            FloatMeaning::from_f32(value.to_f32_with_rounding(direction.ieee()))
        } else if format == FloatFormat::BINARY64 {
            FloatMeaning::from_f64(value.to_f64_with_rounding(direction.ieee()))
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    pub fn convert(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, value.to_exact())
    }

    pub fn convert_toward_zero(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact_directed(format, value.to_exact(), RoundingDirection::TowardZero)
    }

    pub fn convert_toward_positive(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact_directed(format, value.to_exact(), RoundingDirection::TowardPositive)
    }

    pub fn convert_toward_negative(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact_directed(format, value.to_exact(), RoundingDirection::TowardNegative)
    }

    pub fn add(format: FloatFormat, left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().add(&right.to_exact()))
    }

    pub fn add_toward_zero(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::add,
            RoundingDirection::TowardZero,
        )
    }

    pub fn from_integer(format: FloatFormat, value: &BigInt) -> FloatMeaning {
        Self::round_exact(
            format,
            ExactFloat::Finite(BigRational::from_integer(value.clone())),
        )
    }

    pub fn from_integer_toward_zero(format: FloatFormat, value: &BigInt) -> FloatMeaning {
        Self::from_integer_directed(format, value, RoundingDirection::TowardZero)
    }

    pub fn from_integer_toward_positive(format: FloatFormat, value: &BigInt) -> FloatMeaning {
        Self::from_integer_directed(format, value, RoundingDirection::TowardPositive)
    }

    pub fn from_integer_toward_negative(format: FloatFormat, value: &BigInt) -> FloatMeaning {
        Self::from_integer_directed(format, value, RoundingDirection::TowardNegative)
    }

    /// Exact conversion is contract-gated: callers prove this succeeds before
    /// selecting the operation.
    pub fn to_integer_exact(
        value: &FloatMeaning,
        target: IntegerFormat,
    ) -> Result<BigInt, FloatToIntegerError> {
        Self::checked_integer_result(value, target)
    }

    pub fn to_integer_trapping(
        value: &FloatMeaning,
        target: IntegerFormat,
    ) -> Result<BigInt, FloatToIntegerError> {
        Self::checked_integer_result(value, target)
    }

    pub fn to_integer_saturating(value: &FloatMeaning, target: IntegerFormat) -> BigInt {
        let (minimum, maximum) = target.bounds();
        let truncated = match value {
            FloatMeaning::NaN | FloatMeaning::Zero { .. } => return BigInt::zero(),
            FloatMeaning::Infinity { negative: true } => return minimum,
            FloatMeaning::Infinity { negative: false } => return maximum,
            FloatMeaning::FiniteNonZero(value) => value.truncate_to_integer(),
        };
        truncated.clamp(minimum, maximum)
    }

    pub fn add_toward_positive(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::add,
            RoundingDirection::TowardPositive,
        )
    }

    pub fn add_toward_negative(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::add,
            RoundingDirection::TowardNegative,
        )
    }

    pub fn subtract(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().sub(&right.to_exact()))
    }

    pub fn subtract_toward_zero(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::sub,
            RoundingDirection::TowardZero,
        )
    }

    pub fn subtract_toward_positive(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::sub,
            RoundingDirection::TowardPositive,
        )
    }

    pub fn subtract_toward_negative(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::sub,
            RoundingDirection::TowardNegative,
        )
    }

    pub fn multiply(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().mul(&right.to_exact()))
    }

    pub fn multiply_toward_zero(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::mul,
            RoundingDirection::TowardZero,
        )
    }

    pub fn multiply_toward_positive(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::mul,
            RoundingDirection::TowardPositive,
        )
    }

    pub fn multiply_toward_negative(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::mul,
            RoundingDirection::TowardNegative,
        )
    }

    pub fn divide(format: FloatFormat, left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().div(&right.to_exact()))
    }

    pub fn divide_toward_zero(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::div,
            RoundingDirection::TowardZero,
        )
    }

    pub fn divide_toward_positive(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::div,
            RoundingDirection::TowardPositive,
        )
    }

    pub fn divide_toward_negative(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_binary_directed(
            format,
            left,
            right,
            ExactFloat::div,
            RoundingDirection::TowardNegative,
        )
    }

    pub fn negate(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, value.to_exact().negate())
    }

    pub fn square_root(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::NearestTiesToEven)
    }

    pub fn square_root_toward_zero(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::TowardZero)
    }

    pub fn square_root_toward_positive(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::TowardPositive)
    }

    pub fn square_root_toward_negative(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::square_root_directed(format, value, RoundingDirection::TowardNegative)
    }

    pub fn multiply_then_add(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        let product = Self::multiply(format, left, right);
        Self::add(format, &product, addend)
    }

    pub fn fused_multiply_add(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        let exact_product = left.to_exact().mul(&right.to_exact());
        Self::round_exact(format, exact_product.add(&addend.to_exact()))
    }

    pub fn fused_multiply_add_toward_zero(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        Self::fused_multiply_add_directed(
            format,
            left,
            right,
            addend,
            RoundingDirection::TowardZero,
        )
    }

    pub fn fused_multiply_add_toward_positive(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        Self::fused_multiply_add_directed(
            format,
            left,
            right,
            addend,
            RoundingDirection::TowardPositive,
        )
    }

    pub fn fused_multiply_add_toward_negative(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
    ) -> FloatMeaning {
        Self::fused_multiply_add_directed(
            format,
            left,
            right,
            addend,
            RoundingDirection::TowardNegative,
        )
    }

    pub fn classify(format: FloatFormat, value: &FloatMeaning) -> FloatClass {
        match value {
            FloatMeaning::NaN => FloatClass::NaN,
            FloatMeaning::Infinity { negative } => FloatClass::Infinity {
                negative: *negative,
            },
            FloatMeaning::Zero { negative } => FloatClass::Zero {
                negative: *negative,
            },
            FloatMeaning::FiniteNonZero(_) => {
                let (subnormal, negative) = if format == FloatFormat::BINARY32 {
                    let value = value.to_f32();
                    (value.is_subnormal(), value.is_sign_negative())
                } else if format == FloatFormat::BINARY64 {
                    let value = value.to_f64();
                    (value.is_subnormal(), value.is_sign_negative())
                } else {
                    panic!("unsupported floating-point format record: {format:?}")
                };
                if subnormal {
                    FloatClass::Subnormal { negative }
                } else {
                    FloatClass::Normal { negative }
                }
            }
        }
    }

    pub fn is_finite(value: &FloatMeaning) -> bool {
        value.is_finite()
    }

    pub fn is_nan(value: &FloatMeaning) -> bool {
        value.is_nan()
    }

    pub fn is_infinite(value: &FloatMeaning) -> bool {
        value.is_infinite()
    }

    pub fn is_normal(format: FloatFormat, value: &FloatMeaning) -> bool {
        matches!(Self::classify(format, value), FloatClass::Normal { .. })
    }

    pub fn is_subnormal(format: FloatFormat, value: &FloatMeaning) -> bool {
        matches!(Self::classify(format, value), FloatClass::Subnormal { .. })
    }

    /// Apply the result-checked `Trapping` policy. This deliberately checks
    /// only the semantic result: propagating a pre-existing NaN or infinity is
    /// still a non-finite result and therefore traps.
    pub fn apply_trapping_policy(result: FloatMeaning) -> Result<FloatMeaning, FloatPolicyTrap> {
        match result {
            FloatMeaning::NaN => Err(FloatPolicyTrap::NaNResult),
            FloatMeaning::Infinity { .. } => Err(FloatPolicyTrap::InfinityResult),
            finite => Ok(finite),
        }
    }

    /// Apply overflow-only `Saturating` to a semantic operation result.
    /// Infinity clamps only when every operand is finite, which identifies
    /// magnitude overflow for the non-division float operations. Invalid NaN
    /// and non-finite propagation remain unchanged. Division uses
    /// [`Self::apply_saturating_divide_policy`] so a signed-zero divisor is
    /// excluded explicitly.
    pub fn apply_saturating_policy(
        format: FloatFormat,
        operands: &[&FloatMeaning],
        result: FloatMeaning,
    ) -> FloatMeaning {
        if result.is_infinite() && operands.iter().all(|operand| operand.is_finite()) {
            Self::maximum_finite(format, result.is_negative())
        } else {
            result
        }
    }

    /// Division shares overflow-only saturation, but a finite nonzero dividend
    /// divided by signed zero produces infinity without magnitude overflow and
    /// must remain non-finite.
    pub fn apply_saturating_divide_policy(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        result: FloatMeaning,
    ) -> FloatMeaning {
        if right.is_zero() {
            result
        } else {
            Self::apply_saturating_policy(format, &[left, right], result)
        }
    }

    pub fn equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        left.to_exact().equal_value(&right.to_exact())
    }

    pub fn not_equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        !Self::equal(left, right)
    }

    pub fn less(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        left.to_exact().partial_cmp_value(&right.to_exact()) == Some(Ordering::Less)
    }

    pub fn less_or_equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        matches!(
            left.to_exact().partial_cmp_value(&right.to_exact()),
            Some(Ordering::Less | Ordering::Equal)
        )
    }

    pub fn greater(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        left.to_exact().partial_cmp_value(&right.to_exact()) == Some(Ordering::Greater)
    }

    pub fn greater_or_equal(left: &FloatMeaning, right: &FloatMeaning) -> bool {
        matches!(
            left.to_exact().partial_cmp_value(&right.to_exact()),
            Some(Ordering::Greater | Ordering::Equal)
        )
    }

    fn maximum_finite(format: FloatFormat, negative: bool) -> FloatMeaning {
        if format == FloatFormat::BINARY32 {
            let sign = if negative { 0x8000_0000 } else { 0 };
            FloatMeaning::from_f32(f32::from_bits(sign | 0x7f7f_ffff))
        } else if format == FloatFormat::BINARY64 {
            let sign = if negative { 0x8000_0000_0000_0000 } else { 0 };
            FloatMeaning::from_f64(f64::from_bits(sign | 0x7fef_ffff_ffff_ffff))
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    /// Operational min/max law: return the second operand on unordered or
    /// equal, matching the settled native lowering contract.
    pub fn minimum(left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        if Self::less(left, right) {
            left.clone()
        } else {
            right.clone()
        }
    }

    pub fn maximum(left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        if Self::greater(left, right) {
            left.clone()
        } else {
            right.clone()
        }
    }

    fn round_binary_directed(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        operation: fn(&ExactFloat, &ExactFloat) -> ExactFloat,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        let exact = operation(&left.to_exact(), &right.to_exact());
        Self::round_exact_directed(format, exact, direction)
    }

    fn from_integer_directed(
        format: FloatFormat,
        value: &BigInt,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        Self::round_exact_directed(
            format,
            ExactFloat::Finite(BigRational::from_integer(value.clone())),
            direction,
        )
    }

    fn checked_integer_result(
        value: &FloatMeaning,
        target: IntegerFormat,
    ) -> Result<BigInt, FloatToIntegerError> {
        let truncated = match value {
            FloatMeaning::FiniteNonZero(value) => value.truncate_to_integer(),
            FloatMeaning::Zero { .. } => BigInt::zero(),
            FloatMeaning::Infinity { .. } | FloatMeaning::NaN => {
                return Err(FloatToIntegerError::NonFinite);
            }
        };
        let (minimum, maximum) = target.bounds();
        if truncated < minimum || truncated > maximum {
            Err(FloatToIntegerError::OutOfRange)
        } else {
            Ok(truncated)
        }
    }

    fn square_root_directed(
        format: FloatFormat,
        value: &FloatMeaning,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        let input = match value {
            FloatMeaning::NaN | FloatMeaning::Infinity { negative: true } => {
                return FloatMeaning::NaN;
            }
            FloatMeaning::Infinity { negative: false } => {
                return FloatMeaning::Infinity { negative: false };
            }
            FloatMeaning::Zero { negative } => {
                return FloatMeaning::Zero {
                    negative: *negative,
                };
            }
            FloatMeaning::FiniteNonZero(value) if value.is_negative() => {
                return FloatMeaning::NaN;
            }
            FloatMeaning::FiniteNonZero(value) => value,
        };

        if format == FloatFormat::BINARY32 {
            let bits = correctly_rounded_square_root_bits(
                input,
                u64::from(f32::MAX.to_bits()),
                |bits| match ExactFloat::from_f32(f32::from_bits(bits as u32)) {
                    ExactFloat::Finite(value) => value,
                    _ => unreachable!("positive finite binary32 bits decode to a rational"),
                },
                direction,
            );
            FloatMeaning::from_f32(f32::from_bits(bits as u32))
        } else if format == FloatFormat::BINARY64 {
            let bits = correctly_rounded_square_root_bits(
                input,
                f64::MAX.to_bits(),
                |bits| match ExactFloat::from_f64(f64::from_bits(bits)) {
                    ExactFloat::Finite(value) => value,
                    _ => unreachable!("positive finite binary64 bits decode to a rational"),
                },
                direction,
            );
            FloatMeaning::from_f64(f64::from_bits(bits))
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    fn fused_multiply_add_directed(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
        addend: &FloatMeaning,
        direction: RoundingDirection,
    ) -> FloatMeaning {
        let exact_product = left.to_exact().mul(&right.to_exact());
        Self::round_exact_directed(format, exact_product.add(&addend.to_exact()), direction)
    }
}

fn correctly_rounded_square_root_bits(
    input: &BigRational,
    maximum_finite_bits: u64,
    decode: impl Fn(u64) -> BigRational,
    direction: RoundingDirection,
) -> u64 {
    // Positive IEEE encodings are monotonically ordered by their raw bits.
    // Find the largest representable candidate whose exact square is <= the
    // exact input. This is a format-independent proof search over at most 64
    // bit positions, not a host floating-point square root.
    let mut lower = 0u64;
    let mut upper = maximum_finite_bits;
    while lower < upper {
        let middle = lower + (upper - lower).div_ceil(2);
        let candidate = decode(middle);
        if candidate.mul(&candidate).cmp_value(input) != Ordering::Greater {
            lower = middle;
        } else {
            upper = middle - 1;
        }
    }

    let floor = decode(lower);
    if floor.mul(&floor).cmp_value(input) == Ordering::Equal {
        return lower;
    }
    let ceiling_bits = lower + 1;
    if matches!(
        direction,
        RoundingDirection::TowardZero | RoundingDirection::TowardNegative
    ) {
        return lower;
    }
    if direction == RoundingDirection::TowardPositive {
        return ceiling_bits;
    }

    // Compare sqrt(input) to the exact midpoint without taking a root:
    // sqrt(input) ? (floor + ceiling) / 2
    // iff 4 * input ? (floor + ceiling)^2.
    let ceiling = decode(ceiling_bits);
    let four_input = input.mul(&BigRational::from_integer(BigInt::from_u64(4)));
    let sum = floor.add(&ceiling);
    match four_input.cmp_value(&sum.mul(&sum)) {
        Ordering::Less => lower,
        Ordering::Greater => ceiling_bits,
        Ordering::Equal if lower & 1 == 0 => lower,
        Ordering::Equal => ceiling_bits,
    }
}

#[cfg(test)]
mod tests {
    use crate::bignum::{BigInt, ExactFloat};

    use super::{
        FloatClass, FloatFormat, FloatMeaning, FloatPolicyTrap, FloatSemantics,
        FloatToIntegerError, IntegerFormat, RoundingDirection,
    };

    fn meaning32(value: f32) -> FloatMeaning {
        FloatMeaning::from_f32(value)
    }

    #[test]
    fn format_records_match_the_core_surface() {
        assert_eq!(FloatFormat::BINARY32.precision, 24);
        assert_eq!(FloatFormat::BINARY32.minimum_subnormal_exponent, -149);
        assert_eq!(FloatFormat::BINARY64.precision, 53);
        assert_eq!(FloatFormat::BINARY64.minimum_subnormal_exponent, -1074);
    }

    #[test]
    fn decode_encode_round_trips_non_nan_binary_values() {
        for bits in [
            0u32,
            1,
            0x007f_ffff,
            0x0080_0000,
            0x3f80_0000,
            0x7f7f_ffff,
            0x7f80_0000,
            0x8000_0000,
            0xff80_0000,
        ] {
            let value = f32::from_bits(bits);
            assert_eq!(FloatMeaning::from_f32(value).to_f32().to_bits(), bits);
        }
        for bits in [
            0u64,
            1,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
            0x7ff0_0000_0000_0000,
            0x8000_0000_0000_0000,
            0xfff0_0000_0000_0000,
        ] {
            let value = f64::from_bits(bits);
            assert_eq!(FloatMeaning::from_f64(value).to_f64().to_bits(), bits);
        }
    }

    #[test]
    fn binary32_arithmetic_rounds_at_each_named_operation() {
        let large = meaning32(16_777_216.0);
        let one = meaning32(1.0);
        let rounded_sum = FloatSemantics::add(FloatFormat::BINARY32, &large, &one);
        assert_eq!(rounded_sum.to_f32(), 16_777_216.0);
        let difference = FloatSemantics::subtract(FloatFormat::BINARY32, &rounded_sum, &large);
        assert_eq!(difference.to_f32().to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn specials_and_partial_comparisons_follow_the_settled_laws() {
        let positive_zero = meaning32(0.0);
        let negative_zero = meaning32(-0.0);
        let one = meaning32(1.0);
        let infinity = FloatSemantics::divide(FloatFormat::BINARY32, &one, &positive_zero);
        let negative_infinity = FloatSemantics::divide(FloatFormat::BINARY32, &one, &negative_zero);
        let nan = FloatSemantics::divide(FloatFormat::BINARY32, &positive_zero, &positive_zero);

        assert_eq!(infinity.to_f32(), f32::INFINITY);
        assert_eq!(negative_infinity.to_f32(), f32::NEG_INFINITY);
        assert!(nan.is_nan());
        assert!(!FloatSemantics::equal(&nan, &nan));
        assert!(FloatSemantics::not_equal(&nan, &nan));
        assert!(!FloatSemantics::less(&nan, &one));
        assert!(!FloatSemantics::greater_or_equal(&nan, &one));
        assert_eq!(
            FloatSemantics::minimum(&nan, &one).to_f32().to_bits(),
            one.to_f32().to_bits()
        );
        assert!(FloatSemantics::minimum(&one, &nan).is_nan());
    }

    #[test]
    fn signed_zero_survives_exact_arithmetic_and_rounding() {
        let negative_zero = meaning32(-0.0);
        let sum = FloatSemantics::add(FloatFormat::BINARY32, &negative_zero, &negative_zero);
        assert_eq!(sum.to_f32().to_bits(), (-0.0f32).to_bits());
        let negated = FloatSemantics::negate(FloatFormat::BINARY32, &negative_zero);
        assert_eq!(negated.to_f32().to_bits(), 0.0f32.to_bits());
    }

    #[test]
    fn fused_and_multiply_then_add_are_distinct_operations() {
        let left = meaning32(f32::from_bits(0x3f80_0001));
        let right = left.clone();
        let addend = meaning32(f32::from_bits(0xbf80_0002));
        let unfused =
            FloatSemantics::multiply_then_add(FloatFormat::BINARY32, &left, &right, &addend);
        let fused =
            FloatSemantics::fused_multiply_add(FloatFormat::BINARY32, &left, &right, &addend);

        assert_eq!(unfused.to_f32().to_bits(), 0.0f32.to_bits());
        assert_eq!(
            fused.to_f32().to_bits(),
            left.to_f32()
                .mul_add(right.to_f32(), addend.to_f32())
                .to_bits()
        );
        assert_ne!(fused.to_f32().to_bits(), unfused.to_f32().to_bits());
    }

    #[test]
    fn directed_rounding_is_explicit_and_sign_aware() {
        let positive_halfway = ExactFloat::from_decimal_str("1.000000059604644775390625").unwrap();
        let negative_halfway = ExactFloat::from_decimal_str("-1.000000059604644775390625").unwrap();

        let nearest = FloatSemantics::round_exact(FloatFormat::BINARY32, positive_halfway.clone());
        let upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive_halfway,
            RoundingDirection::TowardPositive,
        );
        let downward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            negative_halfway,
            RoundingDirection::TowardNegative,
        );

        assert_eq!(nearest.to_f32().to_bits(), 1.0f32.to_bits());
        assert_eq!(upward.to_f32().to_bits(), 1.0f32.to_bits() + 1);
        assert_eq!(downward.to_f32().to_bits(), (-1.0f32).to_bits() + 1);
    }

    #[test]
    fn directed_named_arithmetic_rounds_the_exact_operation_result() {
        let one = meaning32(1.0);
        let half_ulp = meaning32(f32::from_bits((127 - 24) << 23));
        let nearest = FloatSemantics::add(FloatFormat::BINARY32, &one, &half_ulp);
        let upward = FloatSemantics::add_toward_positive(FloatFormat::BINARY32, &one, &half_ulp);
        assert_eq!(nearest.to_f32().to_bits(), 1.0f32.to_bits());
        assert_eq!(upward.to_f32().to_bits(), 1.0f32.to_bits() + 1);

        let three = meaning32(3.0);
        let quotient = FloatSemantics::divide_toward_zero(FloatFormat::BINARY32, &one, &three);
        assert_eq!(quotient.to_f32().to_bits(), 0x3eaa_aaaa);
    }

    #[test]
    fn directed_underflow_preserves_the_outward_minimum_subnormal() {
        let minimum_subnormal = ExactFloat::from_f32(f32::from_bits(1));
        let two = ExactFloat::from_decimal_str("2").unwrap();
        let positive_half = minimum_subnormal.div(&two);
        let negative_half = positive_half.negate();

        let nearest = FloatSemantics::round_exact(FloatFormat::BINARY32, positive_half.clone());
        let upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive_half,
            RoundingDirection::TowardPositive,
        );
        let downward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            negative_half,
            RoundingDirection::TowardNegative,
        );

        assert_eq!(nearest.to_f32().to_bits(), 0.0f32.to_bits());
        assert_eq!(upward.to_f32().to_bits(), 1);
        assert_eq!(downward.to_f32().to_bits(), 0x8000_0001);
    }

    #[test]
    fn directed_overflow_chooses_infinity_or_max_finite_by_sign() {
        let positive = ExactFloat::from_decimal_str("1e1000").unwrap();
        let negative = ExactFloat::from_decimal_str("-1e1000").unwrap();

        let toward_zero = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive.clone(),
            RoundingDirection::TowardZero,
        );
        let upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            positive,
            RoundingDirection::TowardPositive,
        );
        let negative_upward = FloatSemantics::round_exact_directed(
            FloatFormat::BINARY32,
            negative,
            RoundingDirection::TowardPositive,
        );

        assert_eq!(toward_zero.to_f32().to_bits(), f32::MAX.to_bits());
        assert_eq!(upward.to_f32(), f32::INFINITY);
        assert_eq!(negative_upward.to_f32().to_bits(), (-f32::MAX).to_bits());
    }

    #[test]
    fn trapping_policy_checks_the_result_including_propagated_nonfinites() {
        let finite = meaning32(1.0);
        let nan = FloatMeaning::NaN;
        let infinity = FloatMeaning::Infinity { negative: false };

        assert_eq!(
            FloatSemantics::apply_trapping_policy(finite.clone()),
            Ok(finite)
        );
        assert_eq!(
            FloatSemantics::apply_trapping_policy(nan),
            Err(FloatPolicyTrap::NaNResult)
        );
        assert_eq!(
            FloatSemantics::apply_trapping_policy(infinity),
            Err(FloatPolicyTrap::InfinityResult)
        );
    }

    #[test]
    fn saturating_policy_clamps_only_finite_operand_magnitude_overflow() {
        let one = meaning32(1.0);
        let zero = meaning32(0.0);
        let infinity = FloatMeaning::Infinity { negative: false };
        let negative_infinity = FloatMeaning::Infinity { negative: true };
        let nan = FloatMeaning::NaN;

        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one],
                infinity.clone(),
            )
            .to_f32()
            .to_bits(),
            f32::MAX.to_bits()
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one],
                negative_infinity,
            )
            .to_f32()
            .to_bits(),
            (-f32::MAX).to_bits()
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&infinity, &one],
                infinity.clone(),
            ),
            infinity
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one],
                nan.clone(),
            ),
            nan
        );
        assert_eq!(
            FloatSemantics::apply_saturating_policy(
                FloatFormat::BINARY32,
                &[&one, &one, &zero],
                FloatMeaning::Infinity { negative: false },
            )
            .to_f32()
            .to_bits(),
            f32::MAX.to_bits()
        );
        assert_eq!(
            FloatSemantics::apply_saturating_divide_policy(
                FloatFormat::BINARY32,
                &one,
                &zero,
                FloatMeaning::Infinity { negative: false },
            ),
            FloatMeaning::Infinity { negative: false }
        );
    }

    #[test]
    fn classification_distinguishes_normal_subnormal_zero_and_specials() {
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::MIN_POSITIVE)),
            FloatClass::Normal { negative: false }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::from_bits(1))),
            FloatClass::Subnormal { negative: false }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(-0.0)),
            FloatClass::Zero { negative: true }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::NEG_INFINITY)),
            FloatClass::Infinity { negative: true }
        );
        assert_eq!(
            FloatSemantics::classify(FloatFormat::BINARY32, &meaning32(f32::NAN)),
            FloatClass::NaN
        );
    }

    #[test]
    fn float_to_integer_operations_share_truncation_and_policy_edges() {
        let negative_fraction = FloatMeaning::from_f64(-0.5);
        let negative_one = FloatMeaning::from_f64(-1.0);
        let positive_overflow = FloatMeaning::from_f64(256.0);
        let nan = FloatMeaning::from_f64(f64::NAN);

        assert_eq!(
            FloatSemantics::to_integer_exact(&negative_fraction, IntegerFormat::U8)
                .unwrap()
                .to_u64(),
            Some(0)
        );
        assert_eq!(
            FloatSemantics::to_integer_exact(&negative_one, IntegerFormat::U8),
            Err(FloatToIntegerError::OutOfRange)
        );
        assert_eq!(
            FloatSemantics::to_integer_trapping(&nan, IntegerFormat::I32),
            Err(FloatToIntegerError::NonFinite)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(&positive_overflow, IntegerFormat::U8).to_u64(),
            Some(255)
        );
        assert_eq!(
            FloatSemantics::to_integer_saturating(&nan, IntegerFormat::I32).to_i64(),
            Some(0)
        );
    }

    #[test]
    fn integer_to_float_conversion_rounds_exact_full_width_values() {
        let maximum_u64 = BigInt::from_u64(u64::MAX);
        let nearest = FloatSemantics::from_integer(FloatFormat::BINARY64, &maximum_u64).to_f64();
        let downward =
            FloatSemantics::from_integer_toward_zero(FloatFormat::BINARY64, &maximum_u64).to_f64();

        assert_eq!(nearest.to_bits(), (u64::MAX as f64).to_bits());
        assert!(downward < nearest);
        assert_eq!(
            FloatSemantics::to_integer_saturating(
                &FloatMeaning::from_f64(f64::INFINITY),
                IntegerFormat::U64,
            )
            .to_u64(),
            Some(u64::MAX)
        );
    }

    #[test]
    fn square_root_is_exactly_rounded_without_host_arithmetic() {
        let two32 = meaning32(2.0);
        let nearest32 = FloatSemantics::square_root(FloatFormat::BINARY32, &two32);
        let upward32 = FloatSemantics::square_root_toward_positive(FloatFormat::BINARY32, &two32);
        assert_eq!(nearest32.to_f32().to_bits(), 0x3fb5_04f3);
        assert_eq!(upward32.to_f32().to_bits(), 0x3fb5_04f4);

        let two64 = FloatMeaning::from_f64(2.0);
        let nearest64 = FloatSemantics::square_root(FloatFormat::BINARY64, &two64);
        assert_eq!(nearest64.to_f64().to_bits(), 2.0f64.sqrt().to_bits());

        let perfect = FloatMeaning::from_f64(64.0);
        assert_eq!(
            FloatSemantics::square_root(FloatFormat::BINARY64, &perfect).to_f64(),
            8.0
        );
    }

    #[test]
    fn square_root_preserves_signed_zero_and_rejects_negative_values() {
        let negative_zero = meaning32(-0.0);
        let negative_one = meaning32(-1.0);
        let positive_infinity = meaning32(f32::INFINITY);

        assert_eq!(
            FloatSemantics::square_root(FloatFormat::BINARY32, &negative_zero)
                .to_f32()
                .to_bits(),
            (-0.0f32).to_bits()
        );
        assert!(FloatSemantics::square_root(FloatFormat::BINARY32, &negative_one).is_nan());
        assert_eq!(
            FloatSemantics::square_root(FloatFormat::BINARY32, &positive_infinity).to_f32(),
            f32::INFINITY
        );
    }

    #[test]
    fn square_root_matches_ieee_reference_across_format_boundaries() {
        for bits in [
            1u32,
            2,
            0x007f_ffff,
            0x0080_0000,
            0x3f00_0000,
            0x3f80_0000,
            0x4000_0000,
            0x4b00_0001,
            0x7f7f_ffff,
        ] {
            let value = f32::from_bits(bits);
            let actual =
                FloatSemantics::square_root(FloatFormat::BINARY32, &FloatMeaning::from_f32(value))
                    .to_f32();
            assert_eq!(
                actual.to_bits(),
                value.sqrt().to_bits(),
                "binary32 {bits:#x}"
            );
        }
        for bits in [
            1u64,
            0x000f_ffff_ffff_ffff,
            0x0010_0000_0000_0000,
            0x3fe0_0000_0000_0000,
            0x3ff0_0000_0000_0000,
            0x4000_0000_0000_0000,
            0x7fef_ffff_ffff_ffff,
        ] {
            let value = f64::from_bits(bits);
            let actual =
                FloatSemantics::square_root(FloatFormat::BINARY64, &FloatMeaning::from_f64(value))
                    .to_f64();
            assert_eq!(
                actual.to_bits(),
                value.sqrt().to_bits(),
                "binary64 {bits:#x}"
            );
        }
    }
}
