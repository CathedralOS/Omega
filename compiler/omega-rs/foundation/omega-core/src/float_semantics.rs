//! Executable, host-independent floating-point meanings.
//!
//! Operations decode their landed operands to exact rational/special-value
//! meanings, perform exact arithmetic, and round once through the selected
//! format record. The interpreter, constant folder, proof layer, and target
//! validation can therefore share one definition instead of independently
//! inheriting the host process's floating-point behavior.

use std::cmp::Ordering;

use crate::bignum::{BigRational, ExactFloat};

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
        if format == FloatFormat::BINARY32 {
            FloatMeaning::from_f32(value.to_f32())
        } else if format == FloatFormat::BINARY64 {
            FloatMeaning::from_f64(value.to_f64())
        } else {
            panic!("unsupported floating-point format record: {format:?}")
        }
    }

    pub fn convert(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, value.to_exact())
    }

    pub fn add(format: FloatFormat, left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().add(&right.to_exact()))
    }

    pub fn subtract(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().sub(&right.to_exact()))
    }

    pub fn multiply(
        format: FloatFormat,
        left: &FloatMeaning,
        right: &FloatMeaning,
    ) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().mul(&right.to_exact()))
    }

    pub fn divide(format: FloatFormat, left: &FloatMeaning, right: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, left.to_exact().div(&right.to_exact()))
    }

    pub fn negate(format: FloatFormat, value: &FloatMeaning) -> FloatMeaning {
        Self::round_exact(format, value.to_exact().negate())
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
}

#[cfg(test)]
mod tests {
    use super::{FloatFormat, FloatMeaning, FloatSemantics};

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
}
