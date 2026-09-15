//! The arbitrary-precision rational.

use crate::bignum::exact_float::{pow10, rounded_scaled_quotient, rounding_overflows_to_infinity};
use crate::bignum::{BigInt, IeeeRounding};
use std::cmp::Ordering;
use std::fmt;

/// An exact rational for anonymous decimal constants. The denominator is
/// always positive and the pair is gcd-reduced. It deliberately lives beside
/// `BigInt`: proof facts and const evaluation share one dependency-free exact
/// arithmetic substrate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigRational {
    numerator: BigInt,
    denominator: BigInt,
    negative_zero: bool,
}

impl BigRational {
    pub fn zero() -> Self {
        Self {
            numerator: BigInt::zero(),
            denominator: BigInt::from_u64(1),
            negative_zero: false,
        }
    }

    pub fn from_decimal_str(text: &str) -> Option<Self> {
        let (negative, unsigned) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text.strip_prefix('+').unwrap_or(text)),
        };
        let (mantissa, exponent) = match unsigned.split_once(['e', 'E']) {
            Some((mantissa, exponent)) => (mantissa, exponent.parse::<i64>().ok()?),
            None => (unsigned, 0),
        };
        let (whole, fraction) = match mantissa.split_once('.') {
            Some(parts) => parts,
            None => (mantissa, ""),
        };
        let digits = format!("{whole}{fraction}");
        if digits.is_empty() || !digits.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        let mut numerator = BigInt::from_decimal_str(&digits)?;
        let scale = exponent.checked_sub(i64::try_from(fraction.len()).ok()?)?;
        let denominator;
        if scale >= 0 {
            numerator = numerator.mul(&pow10(usize::try_from(scale).ok()?));
            denominator = BigInt::from_u64(1);
        } else {
            denominator = pow10(usize::try_from(scale.unsigned_abs()).ok()?);
        }
        if negative {
            numerator = numerator.negate();
        }
        let mut value = Self::new(numerator, denominator)?;
        value.negative_zero = negative && value.numerator.is_zero();
        Some(value)
    }

    pub fn from_integer(value: BigInt) -> Self {
        Self::new(value, BigInt::from_u64(1)).expect("one is a nonzero denominator")
    }

    pub fn add(&self, other: &Self) -> Self {
        let numerator = self
            .numerator
            .mul(&other.denominator)
            .add(&other.numerator.mul(&self.denominator));
        Self::new(numerator, self.denominator.mul(&other.denominator))
            .expect("rational denominators are nonzero")
    }

    pub fn sub(&self, other: &Self) -> Self {
        let numerator = self
            .numerator
            .mul(&other.denominator)
            .sub(&other.numerator.mul(&self.denominator));
        Self::new(numerator, self.denominator.mul(&other.denominator))
            .expect("rational denominators are nonzero")
    }

    pub fn mul(&self, other: &Self) -> Self {
        let mut result = Self::new(
            self.numerator.mul(&other.numerator),
            self.denominator.mul(&other.denominator),
        )
        .expect("rational denominators are nonzero");
        result.negative_zero =
            result.numerator.is_zero() && (self.is_negative() != other.is_negative());
        result
    }

    pub fn cmp_value(&self, other: &Self) -> Ordering {
        self.numerator
            .mul(&other.denominator)
            .cmp(&other.numerator.mul(&self.denominator))
    }

    pub fn div(&self, other: &Self) -> Option<Self> {
        if other.numerator.is_zero() {
            return None;
        }
        let mut result = Self::new(
            self.numerator.mul(&other.denominator),
            self.denominator.mul(&other.numerator),
        )?;
        result.negative_zero =
            result.numerator.is_zero() && (self.is_negative() != other.is_negative());
        Some(result)
    }

    pub fn is_zero(&self) -> bool {
        self.numerator.is_zero()
    }

    pub fn is_negative(&self) -> bool {
        self.numerator.is_negative() || (self.numerator.is_zero() && self.negative_zero)
    }

    /// Return the integer value only when the normalized rational has no
    /// fractional part. Unlike a conversion cast, this never truncates.
    pub fn to_integer_exact(&self) -> Option<BigInt> {
        (self.denominator == BigInt::from_u64(1)).then(|| self.numerator.clone())
    }

    /// Borrow the reduced numerator and positive denominator without rounding.
    /// Signed zero has numerator zero; its sign remains separate metadata.
    pub fn as_integer_ratio(&self) -> (&BigInt, &BigInt) {
        (&self.numerator, &self.denominator)
    }

    /// Truncate toward zero, matching the language's float-to-integer
    /// conversion rule.
    pub fn truncate_to_integer(&self) -> BigInt {
        self.numerator
            .div_rem(&self.denominator)
            .expect("a rational denominator is nonzero")
            .0
    }

    pub fn negate(&self) -> Self {
        let mut result = self.clone();
        if result.numerator.is_zero() {
            result.negative_zero = !result.negative_zero;
        } else {
            result.numerator = result.numerator.negate();
        }
        result
    }

    pub fn to_f32(self) -> f32 {
        self.to_f32_with_rounding(IeeeRounding::NearestTiesToEven)
    }

    pub fn to_f64(self) -> f64 {
        self.to_f64_with_rounding(IeeeRounding::NearestTiesToEven)
    }

    pub fn to_f32_with_rounding(self, rounding: IeeeRounding) -> f32 {
        f32::from_bits(self.to_ieee_bits(8, 23, rounding) as u32)
    }

    pub fn to_f64_with_rounding(self, rounding: IeeeRounding) -> f64 {
        f64::from_bits(self.to_ieee_bits(11, 52, rounding))
    }

    pub(crate) fn new(mut numerator: BigInt, mut denominator: BigInt) -> Option<Self> {
        if denominator.is_zero() {
            return None;
        }
        if denominator.is_negative() {
            numerator = numerator.negate();
            denominator = denominator.negate();
        }
        if numerator.is_zero() {
            return Some(Self::zero());
        }
        let gcd = numerator.gcd(&denominator);
        let numerator = numerator.div_rem(&gcd)?.0;
        let denominator = denominator.div_rem(&gcd)?.0;
        Some(Self {
            numerator,
            denominator,
            negative_zero: false,
        })
    }

    pub(crate) fn signed_zero(negative: bool) -> Self {
        let mut result = Self::zero();
        result.negative_zero = negative;
        result
    }

    fn to_ieee_bits(&self, exponent_bits: u32, fraction_bits: u32, rounding: IeeeRounding) -> u64 {
        let sign_shift = exponent_bits + fraction_bits;
        let sign = u64::from(self.is_negative()) << sign_shift;
        if self.numerator.is_zero() {
            return sign;
        }

        let numerator = self.numerator.abs();
        let denominator = &self.denominator;
        let bias = (1i32 << (exponent_bits - 1)) - 1;
        let minimum_exponent = 1 - bias;
        let maximum_exponent = bias;
        let precision = fraction_bits + 1;

        let mut exponent = numerator.bit_length() as i32 - denominator.bit_length() as i32;
        if exponent >= 0 {
            if numerator < denominator.shl_bits(exponent as usize) {
                exponent -= 1;
            }
        } else if numerator.shl_bits(exponent.unsigned_abs() as usize) < *denominator {
            exponent -= 1;
        }

        let (mut significand, subnormal) = if exponent < minimum_exponent {
            (
                rounded_scaled_quotient(
                    &numerator,
                    denominator,
                    (fraction_bits as i32 - minimum_exponent) as isize,
                    self.is_negative(),
                    rounding,
                ),
                true,
            )
        } else {
            (
                rounded_scaled_quotient(
                    &numerator,
                    denominator,
                    (precision as i32 - 1 - exponent) as isize,
                    self.is_negative(),
                    rounding,
                ),
                false,
            )
        };

        if subnormal {
            if significand == 0 {
                return sign;
            }
            let minimum_normal = 1u64 << fraction_bits;
            if significand >= minimum_normal {
                return sign | (1u64 << fraction_bits);
            }
            return sign | significand;
        }

        let carry = 1u64 << precision;
        if significand == carry {
            significand >>= 1;
            exponent += 1;
        }
        if exponent > maximum_exponent {
            let all_exponent_bits = (1u64 << exponent_bits) - 1;
            return if rounding_overflows_to_infinity(self.is_negative(), rounding) {
                sign | (all_exponent_bits << fraction_bits)
            } else {
                let maximum_finite_exponent = all_exponent_bits - 1;
                let maximum_finite_fraction = (1u64 << fraction_bits) - 1;
                sign | (maximum_finite_exponent << fraction_bits) | maximum_finite_fraction
            };
        }
        let exponent_field = u64::try_from(exponent + bias).expect("normal exponent is positive");
        let fraction = significand - (1u64 << fraction_bits);
        sign | (exponent_field << fraction_bits) | fraction
    }
}

impl fmt::Display for BigRational {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.numerator.is_zero() {
            return formatter.write_str("0");
        }
        write!(formatter, "{}", self.numerator)?;
        if self.denominator != BigInt::from_u64(1) {
            write!(formatter, "/{}", self.denominator)?;
        }
        Ok(())
    }
}
