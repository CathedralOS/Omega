//! Exact unbounded signed integers for the PROOF ENGINES (math roster N2).
//! Fact evaluation must be exact: a coefficient that overflows a fixed
//! width silently downgrades a provable goal to "unknown" (or worse,
//! caps a u64 bound at i64::MAX -- the long-standing literal-width
//! debt). Engine coefficients ride this type instead; runtime arithmetic
//! keeps its declared machine widths and never touches it.
//!
//! Sign-magnitude over little-endian u64 limbs. Invariants: no trailing
//! zero limbs; zero is the empty magnitude with `negative == false`
//! (ZII: `Default` is a true zero). Hand-rolled on purpose -- the
//! workspace carries no numeric dependencies, and the engine needs only
//! ring ops, comparison, division, and decimal I/O.

use std::cmp::Ordering;
use std::fmt;

/// The four IEEE rounding directions used by executable float operations.
///
/// This is an explicit operation input inside the semantic engine, never an
/// ambient host-process mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IeeeRounding {
    NearestTiesToEven,
    TowardZero,
    TowardPositive,
    TowardNegative,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct BigInt {
    negative: bool,
    /// Little-endian limbs, no trailing zeros. Empty = zero.
    magnitude: Vec<u64>,
}

impl BigInt {
    pub fn zero() -> Self {
        Self::default()
    }

    pub fn is_zero(&self) -> bool {
        self.magnitude.is_empty()
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    pub fn bit_length(&self) -> usize {
        self.magnitude
            .last()
            .map(|limb| (self.magnitude.len() - 1) * 64 + (64 - limb.leading_zeros() as usize))
            .unwrap_or(0)
    }

    pub fn shl_bits(&self, bits: usize) -> Self {
        if self.is_zero() || bits == 0 {
            return self.clone();
        }
        let word_shift = bits / 64;
        let bit_shift = bits % 64;
        let mut magnitude = vec![0; word_shift];
        let mut carry = 0u64;
        for limb in &self.magnitude {
            if bit_shift == 0 {
                magnitude.push(*limb);
            } else {
                magnitude.push((*limb << bit_shift) | carry);
                carry = *limb >> (64 - bit_shift);
            }
        }
        if carry != 0 {
            magnitude.push(carry);
        }
        Self {
            negative: self.negative,
            magnitude,
        }
        .normalized()
    }

    pub fn from_u64(value: u64) -> Self {
        Self {
            negative: false,
            magnitude: if value == 0 { Vec::new() } else { vec![value] },
        }
    }

    pub fn from_i64(value: i64) -> Self {
        let negative = value < 0;
        let magnitude = value.unsigned_abs();
        Self {
            negative: negative && magnitude != 0,
            magnitude: if magnitude == 0 {
                Vec::new()
            } else {
                vec![magnitude]
            },
        }
    }

    pub fn from_u128(value: u128) -> Self {
        let low = value as u64;
        let high = (value >> 64) as u64;
        let magnitude = match (low, high) {
            (0, 0) => Vec::new(),
            (low, 0) => vec![low],
            (low, high) => vec![low, high],
        };
        Self {
            negative: false,
            magnitude,
        }
    }

    pub fn from_i128(value: i128) -> Self {
        let mut result = Self::from_u128(value.unsigned_abs());
        result.negative = value < 0 && !result.is_zero();
        result
    }

    /// Exact conversion back to i64 when the value fits.
    pub fn to_i64(&self) -> Option<i64> {
        match self.magnitude.len() {
            0 => Some(0),
            1 => {
                let limb = self.magnitude[0];
                if self.negative {
                    (limb <= 1 << 63).then(|| (limb as i64).wrapping_neg())
                } else {
                    i64::try_from(limb).ok()
                }
            }
            _ => None,
        }
    }

    /// Exact conversion back to u64 when the value fits.
    pub fn to_u64(&self) -> Option<u64> {
        match (self.negative, self.magnitude.len()) {
            (false, 0) => Some(0),
            (false, 1) => Some(self.magnitude[0]),
            _ => None,
        }
    }

    /// Nearest-f64 conversion (lossy above 2^53, infinite past f64 range)
    /// -- for float-range DERIVATION only, never exact arithmetic.
    pub fn to_f64_lossy(&self) -> f64 {
        let mut magnitude = 0.0f64;
        for limb in self.magnitude.iter().rev() {
            magnitude = magnitude * 18446744073709551616.0 + *limb as f64;
        }
        if self.negative { -magnitude } else { magnitude }
    }

    pub fn negate(&self) -> Self {
        Self {
            negative: !self.negative && !self.is_zero(),
            magnitude: self.magnitude.clone(),
        }
    }

    pub fn abs(&self) -> Self {
        Self {
            negative: false,
            magnitude: self.magnitude.clone(),
        }
    }

    pub fn add(&self, other: &Self) -> Self {
        if self.negative == other.negative {
            return Self {
                negative: self.negative,
                magnitude: add_magnitudes(&self.magnitude, &other.magnitude),
            }
            .normalized();
        }
        // Opposite signs: subtract the smaller magnitude from the larger;
        // the result takes the larger side's sign.
        match cmp_magnitudes(&self.magnitude, &other.magnitude) {
            Ordering::Equal => Self::zero(),
            Ordering::Greater => Self {
                negative: self.negative,
                magnitude: sub_magnitudes(&self.magnitude, &other.magnitude),
            }
            .normalized(),
            Ordering::Less => Self {
                negative: other.negative,
                magnitude: sub_magnitudes(&other.magnitude, &self.magnitude),
            }
            .normalized(),
        }
    }

    pub fn sub(&self, other: &Self) -> Self {
        self.add(&other.negate())
    }

    pub fn mul(&self, other: &Self) -> Self {
        if self.is_zero() || other.is_zero() {
            return Self::zero();
        }
        Self {
            negative: self.negative != other.negative,
            magnitude: mul_magnitudes(&self.magnitude, &other.magnitude),
        }
        .normalized()
    }

    /// Truncated division with remainder: `self = quotient * other +
    /// remainder`, `|remainder| < |other|`, remainder takes `self`'s sign
    /// (Rust `/`/`%` semantics). Returns None on division by zero.
    pub fn div_rem(&self, other: &Self) -> Option<(Self, Self)> {
        if other.is_zero() {
            return None;
        }
        let (quotient_magnitude, remainder_magnitude) =
            div_rem_magnitudes(&self.magnitude, &other.magnitude);
        let quotient = Self {
            negative: self.negative != other.negative,
            magnitude: quotient_magnitude,
        }
        .normalized();
        let remainder = Self {
            negative: self.negative,
            magnitude: remainder_magnitude,
        }
        .normalized();
        Some((quotient, remainder))
    }

    /// Greatest common divisor, always non-negative. `gcd(0, 0) == 0`.
    pub fn gcd(&self, other: &Self) -> Self {
        let mut a = self.abs();
        let mut b = other.abs();
        while !b.is_zero() {
            let (_, remainder) = a.div_rem(&b).expect("b is nonzero");
            a = b;
            b = remainder.abs();
        }
        a
    }

    /// Parse digits in a radix the lexer produces (2, 8, 10, 16), sign
    /// handled here, no prefix/underscores. Exact at any magnitude.
    pub fn from_str_radix(text: &str, base: u32) -> Option<Self> {
        if base == 10 {
            return Self::from_decimal_str(text);
        }
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        if digits.is_empty() {
            return None;
        }
        let mut magnitude: Vec<u64> = Vec::new();
        for character in digits.chars() {
            let digit = character.to_digit(base)?;
            magnitude = mul_magnitudes_by_u64(&magnitude, u64::from(base));
            magnitude = add_magnitudes(&magnitude, &[u64::from(digit)]);
        }
        Some(
            Self {
                negative,
                magnitude,
            }
            .normalized(),
        )
    }

    pub fn from_decimal_str(text: &str) -> Option<Self> {
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        if digits.is_empty() {
            return None;
        }
        let mut magnitude: Vec<u64> = Vec::new();
        for chunk in DecimalChunks::new(digits) {
            let (chunk_value, chunk_scale) = chunk?;
            magnitude = mul_magnitudes_by_u64(&magnitude, chunk_scale);
            magnitude = add_magnitudes(&magnitude, &[chunk_value]);
        }
        Some(
            Self {
                negative,
                magnitude,
            }
            .normalized(),
        )
    }

    fn normalized(mut self) -> Self {
        while self.magnitude.last() == Some(&0) {
            self.magnitude.pop();
        }
        if self.magnitude.is_empty() {
            self.negative = false;
        }
        self
    }
}

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

    fn new(mut numerator: BigInt, mut denominator: BigInt) -> Option<Self> {
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

    fn signed_zero(negative: bool) -> Self {
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

fn pow10(mut exponent: usize) -> BigInt {
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

fn rounded_scaled_quotient(
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

fn rounding_overflows_to_infinity(negative: bool, rounding: IeeeRounding) -> bool {
    match rounding {
        IeeeRounding::NearestTiesToEven => true,
        IeeeRounding::TowardZero => false,
        IeeeRounding::TowardPositive => !negative,
        IeeeRounding::TowardNegative => negative,
    }
}

impl PartialOrd for BigInt {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BigInt {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.negative, other.negative) {
            (false, true) => Ordering::Greater,
            (true, false) => Ordering::Less,
            (false, false) => cmp_magnitudes(&self.magnitude, &other.magnitude),
            (true, true) => cmp_magnitudes(&other.magnitude, &self.magnitude),
        }
    }
}

impl fmt::Display for BigInt {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_zero() {
            return formatter.write_str("0");
        }
        if self.negative {
            formatter.write_str("-")?;
        }
        // Peel 19-digit decimal chunks off the low end, then print
        // high-to-low (only the leading chunk goes unpadded).
        const CHUNK: u64 = 10_000_000_000_000_000_000; // 10^19
        let mut chunks: Vec<u64> = Vec::new();
        let mut magnitude = self.magnitude.clone();
        while !magnitude.is_empty() {
            let (quotient, remainder) = short_div_rem(&magnitude, CHUNK);
            chunks.push(remainder);
            magnitude = quotient;
        }
        let mut chunks = chunks.into_iter().rev();
        if let Some(leading) = chunks.next() {
            write!(formatter, "{leading}")?;
        }
        for chunk in chunks {
            write!(formatter, "{chunk:019}")?;
        }
        Ok(())
    }
}

/// Compare magnitudes (unsigned).
fn cmp_magnitudes(a: &[u64], b: &[u64]) -> Ordering {
    if a.len() != b.len() {
        return a.len().cmp(&b.len());
    }
    for (limb_a, limb_b) in a.iter().rev().zip(b.iter().rev()) {
        match limb_a.cmp(limb_b) {
            Ordering::Equal => continue,
            unequal => return unequal,
        }
    }
    Ordering::Equal
}

fn add_magnitudes(a: &[u64], b: &[u64]) -> Vec<u64> {
    let (longer, shorter) = if a.len() >= b.len() { (a, b) } else { (b, a) };
    let mut result = Vec::with_capacity(longer.len() + 1);
    let mut carry = 0u64;
    for (index, limb) in longer.iter().enumerate() {
        let (sum, overflow_a) = limb.overflowing_add(carry);
        let (sum, overflow_b) = sum.overflowing_add(*shorter.get(index).unwrap_or(&0));
        carry = u64::from(overflow_a) + u64::from(overflow_b);
        result.push(sum);
    }
    if carry != 0 {
        result.push(carry);
    }
    result
}

/// `a - b`, requiring `a >= b` (callers order operands by magnitude).
fn sub_magnitudes(a: &[u64], b: &[u64]) -> Vec<u64> {
    debug_assert!(cmp_magnitudes(a, b) != Ordering::Less);
    let mut result = Vec::with_capacity(a.len());
    let mut borrow = 0u64;
    for (index, limb) in a.iter().enumerate() {
        let (difference, underflow_a) = limb.overflowing_sub(borrow);
        let (difference, underflow_b) = difference.overflowing_sub(*b.get(index).unwrap_or(&0));
        borrow = u64::from(underflow_a) + u64::from(underflow_b);
        result.push(difference);
    }
    while result.last() == Some(&0) {
        result.pop();
    }
    result
}

fn mul_magnitudes(a: &[u64], b: &[u64]) -> Vec<u64> {
    let mut result = vec![0u64; a.len() + b.len()];
    for (index_a, limb_a) in a.iter().enumerate() {
        if *limb_a == 0 {
            continue;
        }
        let mut carry = 0u128;
        for (index_b, limb_b) in b.iter().enumerate() {
            let product = u128::from(*limb_a) * u128::from(*limb_b)
                + u128::from(result[index_a + index_b])
                + carry;
            result[index_a + index_b] = product as u64;
            carry = product >> 64;
        }
        let mut index = index_a + b.len();
        while carry != 0 {
            let sum = u128::from(result[index]) + carry;
            result[index] = sum as u64;
            carry = sum >> 64;
            index += 1;
        }
    }
    while result.last() == Some(&0) {
        result.pop();
    }
    result
}

fn mul_magnitudes_by_u64(a: &[u64], b: u64) -> Vec<u64> {
    if b == 0 || a.is_empty() {
        return Vec::new();
    }
    mul_magnitudes(a, &[b])
}

/// Shift-subtract long division on magnitudes: `(quotient, remainder)`.
fn div_rem_magnitudes(dividend: &[u64], divisor: &[u64]) -> (Vec<u64>, Vec<u64>) {
    debug_assert!(!divisor.is_empty());
    if cmp_magnitudes(dividend, divisor) == Ordering::Less {
        return (Vec::new(), dividend.to_vec());
    }
    if divisor.len() == 1 {
        let (quotient, remainder) = short_div_rem(dividend, divisor[0]);
        return (
            quotient,
            if remainder == 0 {
                Vec::new()
            } else {
                vec![remainder]
            },
        );
    }
    // Binary long division, most-significant bit first. O(bits * limbs) --
    // engine coefficients are small; correctness over speed.
    let total_bits = dividend.len() * 64;
    let mut quotient = vec![0u64; dividend.len()];
    let mut remainder: Vec<u64> = Vec::new();
    for bit in (0..total_bits).rev() {
        remainder = shift_left_one(&remainder);
        if dividend[bit / 64] >> (bit % 64) & 1 == 1 {
            if remainder.is_empty() {
                remainder.push(1);
            } else {
                remainder[0] |= 1;
            }
        }
        if cmp_magnitudes(&remainder, divisor) != Ordering::Less {
            remainder = sub_magnitudes(&remainder, divisor);
            quotient[bit / 64] |= 1 << (bit % 64);
        }
    }
    while quotient.last() == Some(&0) {
        quotient.pop();
    }
    (quotient, remainder)
}

fn shift_left_one(magnitude: &[u64]) -> Vec<u64> {
    let mut result = Vec::with_capacity(magnitude.len() + 1);
    let mut carry = 0u64;
    for limb in magnitude {
        result.push((limb << 1) | carry);
        carry = limb >> 63;
    }
    if carry != 0 {
        result.push(carry);
    }
    result
}

/// Divide a magnitude by a single limb: `(quotient, remainder)`.
fn short_div_rem(magnitude: &[u64], divisor: u64) -> (Vec<u64>, u64) {
    debug_assert!(divisor != 0);
    let mut quotient = vec![0u64; magnitude.len()];
    let mut remainder = 0u128;
    for index in (0..magnitude.len()).rev() {
        let accumulator = (remainder << 64) | u128::from(magnitude[index]);
        quotient[index] = (accumulator / u128::from(divisor)) as u64;
        remainder = accumulator % u128::from(divisor);
    }
    while quotient.last() == Some(&0) {
        quotient.pop();
    }
    (quotient, remainder as u64)
}

/// Splits a decimal digit string into up-to-19-digit chunks, high first,
/// yielding `(chunk_value, 10^chunk_len)` for the accumulate-multiply loop.
struct DecimalChunks<'text> {
    digits: &'text str,
}

impl<'text> DecimalChunks<'text> {
    fn new(digits: &'text str) -> Self {
        Self { digits }
    }
}

impl Iterator for DecimalChunks<'_> {
    type Item = Option<(u64, u64)>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.digits.is_empty() {
            return None;
        }
        let take = self.digits.len() % 19;
        let take = if take == 0 { 19 } else { take };
        let (chunk, rest) = self.digits.split_at(take);
        self.digits = rest;
        if !chunk.bytes().all(|byte| byte.is_ascii_digit()) {
            return Some(None);
        }
        let value: u64 = match chunk.parse() {
            Ok(value) => value,
            Err(_) => return Some(None),
        };
        Some(Some((value, 10u64.pow(take as u32))))
    }
}

#[cfg(test)]
mod tests {
    use super::{BigInt, BigRational, ExactFloat};

    fn big(value: i128) -> BigInt {
        BigInt::from_i128(value)
    }

    #[test]
    fn zero_is_default_and_signless() {
        assert_eq!(BigInt::default(), BigInt::zero());
        assert_eq!(big(0), big(-0));
        assert!(!big(0).is_negative());
        assert_eq!(big(5).sub(&big(5)), BigInt::zero());
        assert!(!big(5).sub(&big(5)).is_negative());
    }

    #[test]
    fn ring_ops_match_i128_on_a_grid() {
        let samples: &[i128] = &[
            0,
            1,
            -1,
            2,
            -3,
            63,
            64,
            65,
            i128::from(i64::MAX),
            i128::from(i64::MIN),
            i128::from(u64::MAX),
            i128::from(u64::MAX) + 1,
            -(i128::from(u64::MAX) + 7),
            1 << 100,
        ];
        for &a in samples {
            for &b in samples {
                assert_eq!(big(a).add(&big(b)), big(a + b), "{a} + {b}");
                assert_eq!(big(a).sub(&big(b)), big(a - b), "{a} - {b}");
                // Products up to 2^100 * 2^100 overflow i128; guard the oracle.
                if let Some(product) = a.checked_mul(b) {
                    assert_eq!(big(a).mul(&big(b)), big(product), "{a} * {b}");
                }
                assert_eq!(big(a).cmp(&big(b)), a.cmp(&b), "cmp {a} {b}");
                if b != 0 {
                    let (quotient, remainder) = big(a).div_rem(&big(b)).unwrap();
                    assert_eq!(quotient, big(a / b), "{a} / {b}");
                    assert_eq!(remainder, big(a % b), "{a} % {b}");
                }
            }
        }
    }

    #[test]
    fn division_by_zero_is_none() {
        assert!(big(42).div_rem(&big(0)).is_none());
    }

    #[test]
    fn multi_limb_mul_div_round_trips() {
        // (2^100 + 7) * (2^90 + 13), then divide back out.
        let a = big((1 << 100) + 7);
        let b = big((1 << 90) + 13);
        let product = a.mul(&b);
        let (quotient, remainder) = product.div_rem(&a).unwrap();
        assert_eq!(quotient, b);
        assert!(remainder.is_zero());
        let (quotient, remainder) = product.div_rem(&b).unwrap();
        assert_eq!(quotient, a);
        assert!(remainder.is_zero());
    }

    #[test]
    fn gcd_is_euclid() {
        assert_eq!(big(12).gcd(&big(18)), big(6));
        assert_eq!(big(-12).gcd(&big(18)), big(6));
        assert_eq!(big(0).gcd(&big(0)), big(0));
        assert_eq!(big(0).gcd(&big(5)), big(5));
        let a = big(1 << 100);
        assert_eq!(a.gcd(&big(1 << 60)), big(1 << 60));
    }

    #[test]
    fn display_and_parse_round_trip() {
        let cases = [
            "0",
            "1",
            "-1",
            "9223372036854775807",
            "9223372036854775808",
            "18446744073709551615",
            "18446744073709551616",
            "-340282366920938463463374607431768211456",
            "10000000000000000000000000000000000000000000000000000000001",
        ];
        for case in cases {
            let value = BigInt::from_decimal_str(case).unwrap();
            assert_eq!(value.to_string(), case, "round trip {case}");
        }
        assert!(BigInt::from_decimal_str("").is_none());
        assert!(BigInt::from_decimal_str("-").is_none());
        assert!(BigInt::from_decimal_str("12x3").is_none());
        assert_eq!(
            BigInt::from_decimal_str("18446744073709551615").unwrap(),
            BigInt::from_u64(u64::MAX)
        );
        assert_eq!(
            BigInt::from_str_radix("ffffffffffffffff", 16).unwrap(),
            BigInt::from_u64(u64::MAX)
        );
        assert_eq!(
            BigInt::from_str_radix("-ff", 16).unwrap(),
            BigInt::from_i64(-255)
        );
        assert_eq!(
            BigInt::from_str_radix("10000000000000000", 16).unwrap(),
            BigInt::from_u128(1u128 << 64)
        );
        assert_eq!(
            BigInt::from_str_radix("777", 8).unwrap(),
            BigInt::from_i64(0o777)
        );
        assert_eq!(
            BigInt::from_str_radix("1010", 2).unwrap(),
            BigInt::from_i64(10)
        );
        assert!(BigInt::from_str_radix("f", 8).is_none());
    }

    #[test]
    fn narrowing_conversions_are_exact() {
        assert_eq!(big(i128::from(i64::MAX)).to_i64(), Some(i64::MAX));
        assert_eq!(big(i128::from(i64::MIN)).to_i64(), Some(i64::MIN));
        assert_eq!(big(i128::from(i64::MAX) + 1).to_i64(), None);
        assert_eq!(big(i128::from(i64::MIN) - 1).to_i64(), None);
        assert_eq!(big(i128::from(u64::MAX)).to_u64(), Some(u64::MAX));
        assert_eq!(big(i128::from(u64::MAX) + 1).to_u64(), None);
        assert_eq!(big(-1).to_u64(), None);
        assert_eq!(big(0).to_u64(), Some(0));
    }

    #[test]
    fn rational_decimal_arithmetic_stays_exact_until_rounding() {
        let one_tenth = BigRational::from_decimal_str("0.1").unwrap();
        let two_tenths = BigRational::from_decimal_str("0.2").unwrap();
        let exact = one_tenth.add(&two_tenths);
        assert_eq!(exact.to_f64().to_bits(), 0.3f64.to_bits());
        assert_ne!((0.1f64 + 0.2f64).to_bits(), 0.3f64.to_bits());

        let quotient = BigRational::from_decimal_str("1")
            .unwrap()
            .div(&BigRational::from_decimal_str("3").unwrap())
            .unwrap();
        assert_eq!(quotient.to_f32().to_bits(), (1.0f32 / 3.0).to_bits());
    }

    #[test]
    fn rational_rounds_directly_to_binary32_with_ties_to_even() {
        let witness = BigRational::from_decimal_str("8388609.499999999999999").unwrap();
        assert_eq!(witness.to_f32().to_bits(), 0x4b00_0001);

        let halfway = BigRational::from_decimal_str("1.000000059604644775390625").unwrap();
        assert_eq!(halfway.to_f32().to_bits(), 1.0f32.to_bits());
        let above =
            BigRational::from_decimal_str("1.0000000596046447753906250000000000000000000000001")
                .unwrap();
        assert_eq!(above.to_f32().to_bits(), 1.0f32.to_bits() + 1);
    }

    #[test]
    fn rational_ieee_conversion_handles_zero_subnormal_and_overflow() {
        assert_eq!(
            BigRational::from_decimal_str("-0.0")
                .unwrap()
                .to_f32()
                .to_bits(),
            1 << 31
        );
        assert_eq!(
            BigRational::from_decimal_str("1e-50").unwrap().to_f32(),
            0.0
        );
        assert_eq!(
            BigRational::from_decimal_str(
                "1.401298464324817070923729583289916131280261941876515771757068283e-45",
            )
            .unwrap()
            .to_f32()
            .to_bits(),
            1
        );
        assert!(
            BigRational::from_decimal_str("1e100")
                .unwrap()
                .to_f32()
                .is_infinite()
        );
        assert!(
            BigRational::from_decimal_str("1e400")
                .unwrap()
                .to_f64()
                .is_infinite()
        );
    }

    #[test]
    fn exact_float_arithmetic_produces_specials_at_landing() {
        let zero = ExactFloat::from_decimal_str("0.0").unwrap();
        let negative_zero = ExactFloat::from_decimal_str("-0.0").unwrap();
        let one = ExactFloat::from_decimal_str("1.0").unwrap();
        assert!(zero.div(&zero).to_f64().is_nan());
        assert_eq!(one.div(&zero).to_f32(), f32::INFINITY);
        assert_eq!(one.div(&negative_zero).to_f32(), f32::NEG_INFINITY);
        assert!(
            one.div(&zero)
                .add(&one.div(&negative_zero))
                .to_f64()
                .is_nan()
        );
        assert_eq!(
            zero.div(&one.div(&zero)).to_f32().to_bits(),
            0.0f32.to_bits()
        );
    }

    #[test]
    fn rational_decimal_rounding_matches_rust_parsers_on_a_grid() {
        let mut seed = 0x9e37_79b9_7f4a_7c15u64;
        for _ in 0..2_000 {
            seed = seed
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            let whole = seed % 10_000_000_000;
            seed = seed.rotate_left(17) ^ 0xa076_1d64_78bd_642f;
            let fraction = seed % 1_000_000_000;
            let exponent = (seed.rotate_left(11) % 241) as i32 - 120;
            let sign = if seed & 1 == 0 { "" } else { "-" };
            let text = format!("{sign}{whole}.{fraction:09}e{exponent}");
            let exact = BigRational::from_decimal_str(&text).unwrap();
            assert_eq!(
                exact.clone().to_f32().to_bits(),
                text.parse::<f32>().unwrap().to_bits(),
                "binary32 rounding for {text}"
            );
            assert_eq!(
                exact.to_f64().to_bits(),
                text.parse::<f64>().unwrap().to_bits(),
                "binary64 rounding for {text}"
            );
        }
    }
}
