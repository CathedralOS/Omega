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
            magnitude: if magnitude == 0 { Vec::new() } else { vec![magnitude] },
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
    for index in 0..longer.len() {
        let (sum, overflow_a) = longer[index].overflowing_add(carry);
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
    for index in 0..a.len() {
        let (difference, underflow_a) = a[index].overflowing_sub(borrow);
        let (difference, underflow_b) =
            difference.overflowing_sub(*b.get(index).unwrap_or(&0));
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
            if remainder == 0 { Vec::new() } else { vec![remainder] },
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
    use super::BigInt;

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
}
