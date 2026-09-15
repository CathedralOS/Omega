//! The exact float, its scaled rounding and its ordering and display.

use crate::bignum::{BigInt, BigRational, IeeeRounding};
use std::cmp::Ordering;

/// Exact anonymous-float evaluation, including the format-independent special
/// values arithmetic can produce before a concrete IEEE format is requested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExactFloat {
    Finite(BigRational),
    Infinity { negative: bool },
    NaN,
}

impl ExactFloat {
    pub fn from_decimal_str(text: &str) -> Option<Self> {
        match text {
            "NaN" | "+NaN" | "-NaN" | "nan" | "+nan" | "-nan" => Some(Self::NaN),
            "inf" | "+inf" | "infinity" | "+infinity" => Some(Self::Infinity { negative: false }),
            "-inf" | "-infinity" => Some(Self::Infinity { negative: true }),
            _ => BigRational::from_decimal_str(text).map(Self::Finite),
        }
    }

    /// Decode one binary32 value into its exact proof meaning. Finite values
    /// become dyadic rationals; signed zero, infinity, and NaN retain the
    /// distinctions needed by the format-independent semantic operators.
    pub fn from_f32(value: f32) -> Self {
        Self::from_ieee_bits(u64::from(value.to_bits()), 8, 23)
    }

    /// Decode one binary64 value into its exact proof meaning.
    pub fn from_f64(value: f64) -> Self {
        Self::from_ieee_bits(value.to_bits(), 11, 52)
    }

    pub fn add(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => Self::NaN,
            (Self::Infinity { negative: left }, Self::Infinity { negative: right })
                if left != right =>
            {
                Self::NaN
            }
            (Self::Infinity { negative }, _) | (_, Self::Infinity { negative }) => Self::Infinity {
                negative: *negative,
            },
            (Self::Finite(left), Self::Finite(right)) => {
                let mut result = left.add(right);
                if result.is_zero() {
                    result = BigRational::signed_zero(left.is_negative() && right.is_negative());
                }
                Self::Finite(result)
            }
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.negate())
    }

    pub fn mul(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => Self::NaN,
            (Self::Infinity { .. }, Self::Finite(value))
            | (Self::Finite(value), Self::Infinity { .. })
                if value.is_zero() =>
            {
                Self::NaN
            }
            (Self::Infinity { negative: left }, right)
            | (right, Self::Infinity { negative: left }) => Self::Infinity {
                negative: *left != right.is_negative(),
            },
            (Self::Finite(left), Self::Finite(right)) => Self::Finite(left.mul(right)),
        }
    }

    pub fn div(&self, other: &Self) -> Self {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => Self::NaN,
            (Self::Infinity { .. }, Self::Infinity { .. }) => Self::NaN,
            (Self::Finite(left), Self::Finite(right)) if right.is_zero() => {
                if left.is_zero() {
                    Self::NaN
                } else {
                    Self::Infinity {
                        negative: left.is_negative() != right.is_negative(),
                    }
                }
            }
            (Self::Infinity { negative }, Self::Finite(right)) => Self::Infinity {
                negative: *negative != right.is_negative(),
            },
            (Self::Finite(left), Self::Infinity { negative }) => {
                Self::Finite(BigRational::signed_zero(left.is_negative() != *negative))
            }
            (Self::Finite(left), Self::Finite(right)) => {
                Self::Finite(left.div(right).expect("nonzero divisor"))
            }
        }
    }

    pub fn negate(&self) -> Self {
        match self {
            Self::Finite(value) => Self::Finite(value.negate()),
            Self::Infinity { negative } => Self::Infinity {
                negative: !negative,
            },
            Self::NaN => Self::NaN,
        }
    }

    pub fn is_negative(&self) -> bool {
        match self {
            Self::Finite(value) => value.is_negative(),
            Self::Infinity { negative } => *negative,
            Self::NaN => false,
        }
    }

    pub fn equal_value(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => false,
            (Self::Infinity { negative: left }, Self::Infinity { negative: right }) => {
                left == right
            }
            (Self::Finite(left), Self::Finite(right)) => left.cmp_value(right) == Ordering::Equal,
            _ => false,
        }
    }

    pub fn partial_cmp_value(&self, other: &Self) -> Option<Ordering> {
        match (self, other) {
            (Self::NaN, _) | (_, Self::NaN) => None,
            (Self::Infinity { negative: left }, Self::Infinity { negative: right }) => {
                Some(right.cmp(left))
            }
            (Self::Infinity { negative: true }, _) | (_, Self::Infinity { negative: false }) => {
                Some(Ordering::Less)
            }
            (Self::Infinity { negative: false }, _) | (_, Self::Infinity { negative: true }) => {
                Some(Ordering::Greater)
            }
            (Self::Finite(left), Self::Finite(right)) => Some(left.cmp_value(right)),
        }
    }

    pub fn to_f32(&self) -> f32 {
        self.to_f32_with_rounding(IeeeRounding::NearestTiesToEven)
    }

    pub fn to_f32_with_rounding(&self, rounding: IeeeRounding) -> f32 {
        match self {
            Self::Finite(value) => value.clone().to_f32_with_rounding(rounding),
            Self::Infinity { negative: false } => f32::INFINITY,
            Self::Infinity { negative: true } => f32::NEG_INFINITY,
            Self::NaN => f32::NAN,
        }
    }

    pub fn to_f64(&self) -> f64 {
        self.to_f64_with_rounding(IeeeRounding::NearestTiesToEven)
    }

    pub fn to_f64_with_rounding(&self, rounding: IeeeRounding) -> f64 {
        match self {
            Self::Finite(value) => value.clone().to_f64_with_rounding(rounding),
            Self::Infinity { negative: false } => f64::INFINITY,
            Self::Infinity { negative: true } => f64::NEG_INFINITY,
            Self::NaN => f64::NAN,
        }
    }

    fn from_ieee_bits(bits: u64, exponent_bits: u32, fraction_bits: u32) -> Self {
        let sign_shift = exponent_bits + fraction_bits;
        let negative = ((bits >> sign_shift) & 1) != 0;
        let exponent_mask = (1u64 << exponent_bits) - 1;
        let exponent_field = (bits >> fraction_bits) & exponent_mask;
        let fraction_mask = (1u64 << fraction_bits) - 1;
        let fraction = bits & fraction_mask;

        if exponent_field == exponent_mask {
            return if fraction == 0 {
                Self::Infinity { negative }
            } else {
                Self::NaN
            };
        }
        if exponent_field == 0 && fraction == 0 {
            return Self::Finite(BigRational::signed_zero(negative));
        }

        let bias = (1i32 << (exponent_bits - 1)) - 1;
        let (significand, binary_exponent) = if exponent_field == 0 {
            (fraction, 1 - bias - fraction_bits as i32)
        } else {
            (
                (1u64 << fraction_bits) | fraction,
                exponent_field as i32 - bias - fraction_bits as i32,
            )
        };
        let mut numerator = BigInt::from_u64(significand);
        if negative {
            numerator = numerator.negate();
        }
        let (numerator, denominator) = if binary_exponent >= 0 {
            (
                numerator.shl_bits(binary_exponent as usize),
                BigInt::from_u64(1),
            )
        } else {
            (
                numerator,
                BigInt::from_u64(1).shl_bits(binary_exponent.unsigned_abs() as usize),
            )
        };
        Self::Finite(
            BigRational::new(numerator, denominator)
                .expect("an IEEE finite value has a positive denominator"),
        )
    }
}

pub(crate) fn pow10(mut exponent: usize) -> BigInt {
    let mut result = BigInt::from_u64(1);
    let mut base = BigInt::from_u64(10);
    while exponent != 0 {
        if exponent & 1 == 1 {
            result = result.mul(&base);
        }
        exponent >>= 1;
        if exponent != 0 {
            base = base.mul(&base);
        }
    }
    result
}

pub(crate) fn rounded_scaled_quotient(
    numerator: &BigInt,
    denominator: &BigInt,
    binary_shift: isize,
    negative: bool,
    rounding: IeeeRounding,
) -> u64 {
    let (scaled_numerator, scaled_denominator) = if binary_shift >= 0 {
        (
            numerator.shl_bits(binary_shift as usize),
            denominator.clone(),
        )
    } else {
        (
            numerator.clone(),
            denominator.shl_bits(binary_shift.unsigned_abs()),
        )
    };
    let (quotient, remainder) = scaled_numerator
        .div_rem(&scaled_denominator)
        .expect("rational denominator is nonzero");
    let mut quotient = quotient.to_u64().expect("IEEE significand fits in one u64");
    let twice_remainder = remainder.add(&remainder);
    let increment = match rounding {
        IeeeRounding::NearestTiesToEven => {
            twice_remainder > scaled_denominator
                || (twice_remainder == scaled_denominator && quotient & 1 == 1)
        }
        IeeeRounding::TowardZero => false,
        IeeeRounding::TowardPositive => !negative && !remainder.is_zero(),
        IeeeRounding::TowardNegative => negative && !remainder.is_zero(),
    };
    if increment {
        quotient += 1;
    }
    quotient
}

pub(crate) fn rounding_overflows_to_infinity(negative: bool, rounding: IeeeRounding) -> bool {
    match rounding {
        IeeeRounding::NearestTiesToEven => true,
        IeeeRounding::TowardZero => false,
        IeeeRounding::TowardPositive => !negative,
        IeeeRounding::TowardNegative => negative,
    }
}
