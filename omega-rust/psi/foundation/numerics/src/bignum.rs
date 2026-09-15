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
//!
//! This file owns the IEEE rounding mode and the arbitrary-precision
//! integer. `big_rational.rs` carries the rational, `exact_float.rs` the
//! exact float and its rounding, `magnitude_arithmetic.rs` the magnitude
//! arithmetic and decimal chunking shared by all three and `tests.rs` the
//! number tests.

mod big_rational;
mod exact_float;
mod magnitude_arithmetic;
#[cfg(test)]
mod tests;

pub use big_rational::BigRational;
pub use exact_float::ExactFloat;

use crate::bignum::magnitude_arithmetic::{
    DecimalChunks, add_magnitudes, cmp_magnitudes, div_rem_magnitudes, mul_magnitudes,
    mul_magnitudes_by_u64, short_div_rem, sub_magnitudes,
};
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
