//! Magnitude comparison, addition, multiplication, division and decimal
//! chunking.

use std::cmp::Ordering;

/// Compare magnitudes (unsigned).
pub(crate) fn cmp_magnitudes(a: &[u64], b: &[u64]) -> Ordering {
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

pub(crate) fn add_magnitudes(a: &[u64], b: &[u64]) -> Vec<u64> {
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
pub(crate) fn sub_magnitudes(a: &[u64], b: &[u64]) -> Vec<u64> {
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

pub(crate) fn mul_magnitudes(a: &[u64], b: &[u64]) -> Vec<u64> {
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

pub(crate) fn mul_magnitudes_by_u64(a: &[u64], b: u64) -> Vec<u64> {
    if b == 0 || a.is_empty() {
        return Vec::new();
    }
    mul_magnitudes(a, &[b])
}

/// Shift-subtract long division on magnitudes: `(quotient, remainder)`.
pub(crate) fn div_rem_magnitudes(dividend: &[u64], divisor: &[u64]) -> (Vec<u64>, Vec<u64>) {
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
pub(crate) fn short_div_rem(magnitude: &[u64], divisor: u64) -> (Vec<u64>, u64) {
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
pub(crate) struct DecimalChunks<'text> {
    digits: &'text str,
}

impl<'text> DecimalChunks<'text> {
    pub(crate) fn new(digits: &'text str) -> Self {
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
