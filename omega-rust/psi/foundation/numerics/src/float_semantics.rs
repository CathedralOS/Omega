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

    /// Whether `value` lies inside this carrier's declared bounds.
    pub(crate) fn contains(self, value: &BigInt) -> bool {
        let (minimum, maximum) = self.bounds();
        *value >= minimum && *value <= maximum
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
mod tests;
