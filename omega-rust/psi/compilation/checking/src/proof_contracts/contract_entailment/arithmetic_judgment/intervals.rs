//! Interval arithmetic for the arithmetic engine: the endpoint-precise
//! `Interval` lattice and the operator-aware interval rules (remainder,
//! quotient, shifts, bitwise-and) that mint opaque atoms for unknown
//! residues.
use super::super::BigInt;
use super::{Engine, OpaqueTerm, Polynomial};

/// An interval with optional (= unbounded) ends; end arithmetic is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Interval {
    pub(super) low: Option<BigInt>,
    pub(super) high: Option<BigInt>,
}

impl Interval {
    pub(super) fn unbounded() -> Self {
        Self {
            low: None,
            high: None,
        }
    }

    pub(super) fn constant(value: BigInt) -> Self {
        Self {
            low: Some(value.clone()),
            high: Some(value),
        }
    }

    pub(super) fn add(&self, other: &Self) -> Self {
        Self {
            low: match (&self.low, &other.low) {
                (Some(a), Some(b)) => Some(a.add(b)),
                _ => None,
            },
            high: match (&self.high, &other.high) {
                (Some(a), Some(b)) => Some(a.add(b)),
                _ => None,
            },
        }
    }

    pub(super) fn scale(&self, factor: &BigInt) -> Self {
        if factor.is_zero() {
            return Self::constant(BigInt::zero());
        }
        let scaled_low = self.low.as_ref().map(|value| value.mul(factor));
        let scaled_high = self.high.as_ref().map(|value| value.mul(factor));
        if factor.is_negative() {
            Self {
                low: scaled_high,
                high: scaled_low,
            }
        } else {
            Self {
                low: scaled_low,
                high: scaled_high,
            }
        }
    }

    pub(super) fn multiply(&self, other: &Self) -> Self {
        // Scalar multiplication retains a one-sided bound. In particular,
        // the monomial accumulator's initial 1 must not erase [0, +infinity).
        if let (Some(low), Some(high)) = (&self.low, &self.high)
            && low == high
        {
            return other.scale(low);
        }
        if let (Some(low), Some(high)) = (&other.low, &other.high)
            && low == high
        {
            return self.scale(low);
        }
        // An unbounded end makes the product unbounded on the side it could
        // extend; with all four ends finite the corner products are exact.
        let (Some(self_low), Some(self_high), Some(other_low), Some(other_high)) =
            (&self.low, &self.high, &other.low, &other.high)
        else {
            return Interval::unbounded();
        };
        let candidates = [
            self_low.mul(other_low),
            self_low.mul(other_high),
            self_high.mul(other_low),
            self_high.mul(other_high),
        ];
        Self {
            low: candidates.iter().min().cloned(),
            high: candidates.iter().max().cloned(),
        }
    }

    /// `self` raised to `power`, treating repeated factors as CORRELATED:
    /// an even power of any interval is non-negative, and the square of
    /// `[lo, hi]` is exact rather than the independent product.
    pub(super) fn correlated_power(&self, power: u32) -> Self {
        if power == 0 {
            return Self::constant(BigInt::from_i64(1));
        }
        if power == 1 {
            return self.clone();
        }
        let (Some(low), Some(high)) = (&self.low, &self.high) else {
            // Unbounded base: an even power is still known non-negative.
            return if power.is_multiple_of(2) {
                Self {
                    low: Some(BigInt::zero()),
                    high: None,
                }
            } else {
                Interval::unbounded()
            };
        };
        let corner_low = pow(low, power);
        let corner_high = pow(high, power);
        if power % 2 == 1 {
            return Self {
                low: Some(corner_low),
                high: Some(corner_high),
            };
        }
        let max_corner = corner_low.clone().max(corner_high.clone());
        let min_corner = if !low.is_negative() || high.is_negative() {
            corner_low.min(corner_high)
        } else {
            // The base interval straddles zero: the even power bottoms at 0.
            BigInt::zero()
        };
        Self {
            low: Some(min_corner),
            high: Some(max_corner),
        }
    }
}

pub(super) fn pow(base: &BigInt, power: u32) -> BigInt {
    let mut result = BigInt::from_i64(1);
    for _ in 0..power {
        result = result.mul(base);
    }
    result
}

/// `floor(value / 2^count)`: an exact `>>`'s mathematical value. The engine's
/// `div_rem` truncates toward zero, so a negative nondivisible value drops
/// one more -- `-3 >> 1` is `-2`, not the truncated quotient `-1`.
pub(super) fn floor_shift_right(value: &BigInt, count: u64) -> Option<BigInt> {
    let divisor = BigInt::from_i64(1).shl_bits(count as usize);
    let (quotient, remainder) = value.div_rem(&divisor)?;
    Some(if remainder.is_negative() {
        quotient.sub(&BigInt::from_i64(1))
    } else {
        quotient
    })
}

/// The representable `[minimum, maximum]` of a `bits`-wide integer carrier,
/// two's complement when `signed`. `bits` names a real machine width
/// (8..=64), so the shifts below cannot fail.
pub(super) fn bitwise_carrier_bounds(bits: u32, signed: bool) -> (BigInt, BigInt) {
    if signed {
        let half = BigInt::from_i64(1).shl_bits((bits - 1) as usize);
        (half.negate(), half.sub(&BigInt::from_i64(1)))
    } else {
        (
            BigInt::zero(),
            BigInt::from_i64(1)
                .shl_bits(bits as usize)
                .sub(&BigInt::from_i64(1)),
        )
    }
}

/// `left & right` inside a `bits`-wide two's-complement carrier. Each
/// operand reduces modulo `2^bits` -- every real operand is already
/// in-carrier, so the reduction is identity there and only keeps a spelled
/// out-of-range constant total -- then the u64 magnitudes AND and the
/// result reinterprets under `signed`.
pub(super) fn bitwise_and_constants(
    left: &BigInt,
    right: &BigInt,
    bits: u32,
    signed: bool,
) -> Option<BigInt> {
    let modulus = BigInt::from_i64(1).shl_bits(bits as usize);
    let reduce = |value: &BigInt| -> Option<u64> {
        let (_, remainder) = value.div_rem(&modulus)?;
        let reduced = if remainder.is_negative() {
            remainder.add(&modulus)
        } else {
            remainder
        };
        reduced.to_u64()
    };
    let masked = reduce(left)? & reduce(right)?;
    let value = BigInt::from_u64(masked);
    // A signed carrier reinterprets the top bit; `bits <= 64` always, and
    // for `bits == 64` that bit is position 63.
    if signed && masked & (1u64 << (bits - 1).min(63)) != 0 {
        Some(value.sub(&modulus))
    } else {
        Some(value)
    }
}

impl<'program> Engine<'program> {
    /// The interval a truncating remainder atom takes from its operand's:
    /// `(-|modulus|, |modulus|)` clamped by the operand's own sign, tightening
    /// monotonically when the operand's whole range shares one quotient and
    /// the mint site admitted that reading.
    pub(super) fn remainder_interval(
        &self,
        operand: &Polynomial,
        modulus: &Polynomial,
        tight_interval: bool,
    ) -> Interval {
        let modulus = self.substituted(modulus);
        if let Some(constant) = modulus.constant_value() {
            if constant.is_zero() {
                return Interval::unbounded();
            }
            return self.constant_remainder_interval(operand, &constant, tight_interval);
        }
        let operand_interval = self.polynomial_interval(&self.substituted(operand));
        let divisor_interval = self.polynomial_interval(&modulus);
        // Truncation keeps the dividend's sign and never increases its
        // magnitude. A bounded runtime divisor additionally gives |r| < |d|.
        // Nonzero and machine-width formation remain separate obligations.
        let magnitude = divisor_interval
            .low
            .zip(divisor_interval.high)
            .map(|(low, high)| {
                low.abs()
                    .max(high.abs())
                    .sub(&BigInt::from_i64(1))
                    .max(BigInt::zero())
            });
        let low = operand_interval.low.map(|value| value.min(BigInt::zero()));
        let high = operand_interval.high.map(|value| value.max(BigInt::zero()));
        Interval {
            low: match (&magnitude, low) {
                (Some(magnitude), Some(low)) => Some(low.max(magnitude.negate())),
                (Some(magnitude), None) => Some(magnitude.negate()),
                (None, low) => low,
            },
            high: match (magnitude, high) {
                (Some(magnitude), Some(high)) => Some(high.min(magnitude)),
                (Some(magnitude), None) => Some(magnitude),
                (None, high) => high,
            },
        }
    }

    pub(super) fn constant_remainder_interval(
        &self,
        operand: &Polynomial,
        modulus: &BigInt,
        tight_interval: bool,
    ) -> Interval {
        let magnitude = modulus.abs().sub(&BigInt::from_i64(1));
        let operand_interval = self.polynomial_interval(&self.substituted(operand));
        if tight_interval
            && let (Some(low), Some(high)) = (&operand_interval.low, &operand_interval.high)
            && let (Some((low_quotient, low_remainder)), Some((high_quotient, high_remainder))) =
                (low.div_rem(modulus), high.div_rem(modulus))
            && low_quotient == high_quotient
        {
            // A constant quotient makes remainder increase with the
            // dividend, including when the divisor is negative.
            return Interval {
                low: Some(low_remainder),
                high: Some(high_remainder),
            };
        }
        Interval {
            low: Some(operand_interval.low.map_or_else(
                || magnitude.negate(),
                |minimum| minimum.min(BigInt::zero()).max(magnitude.negate()),
            )),
            high: Some(operand_interval.high.map_or_else(
                || magnitude.clone(),
                |maximum| maximum.max(BigInt::zero()).min(magnitude.clone()),
            )),
        }
    }

    /// The interval a truncating quotient atom takes from its dividend's.
    pub(super) fn quotient_interval(
        &self,
        dividend: &Polynomial,
        divisor: &Polynomial,
    ) -> Interval {
        let dividend_interval = self.polynomial_interval(&self.substituted(dividend));
        let divisor = self.substituted(divisor);
        if let Some(divisor) = divisor.constant_value() {
            return Self::constant_quotient_interval(dividend_interval, &divisor);
        }
        let divisor_interval = self.polynomial_interval(&divisor);
        Self::bounded_quotient_interval(&dividend_interval, &divisor_interval)
    }

    pub(super) fn constant_quotient_interval(dividend: Interval, divisor: &BigInt) -> Interval {
        let quotient_bound = |bound: Option<BigInt>| {
            bound.and_then(|value| value.div_rem(divisor).map(|(quotient, _)| quotient))
        };
        if divisor.is_negative() {
            Interval {
                low: quotient_bound(dividend.high),
                high: quotient_bound(dividend.low),
            }
        } else {
            Interval {
                low: quotient_bound(dividend.low),
                high: quotient_bound(dividend.high),
            }
        }
    }

    pub(super) fn bounded_quotient_interval(dividend: &Interval, divisor: &Interval) -> Interval {
        let (Some(dividend_low), Some(dividend_high)) = (&dividend.low, &dividend.high) else {
            return Interval::unbounded();
        };
        // On a rectangle whose denominator has one strict sign, real
        // division reaches its extrema at corners. Truncation toward zero
        // is monotone, so the same corners bound the integer quotient.
        // A zero-inclusive interval supplies no quotient bounds, even if an
        // enclosing polynomial cancels the opaque term.
        if !divisor
            .low
            .as_ref()
            .is_some_and(|low| low > &BigInt::zero())
            && !divisor
                .high
                .as_ref()
                .is_some_and(|high| high < &BigInt::zero())
        {
            return Interval::unbounded();
        }
        // When the denominator extends to infinity on its admitted side,
        // the quotient tends to zero. Include that limit, not an invented
        // finite cap or the source carrier's analysis-window approximation.
        let corner = |numerator: &BigInt, denominator: &Option<BigInt>| match denominator {
            Some(denominator) => numerator.div_rem(denominator).map(|(quotient, _)| quotient),
            None => Some(BigInt::zero()),
        };
        let [
            Some(low_low),
            Some(low_high),
            Some(high_low),
            Some(high_high),
        ] = [
            corner(dividend_low, &divisor.low),
            corner(dividend_low, &divisor.high),
            corner(dividend_high, &divisor.low),
            corner(dividend_high, &divisor.high),
        ]
        else {
            return Interval::unbounded();
        };
        let quotients = [low_low, low_high, high_low, high_high];
        Interval {
            low: quotients.iter().min().cloned(),
            high: quotients.iter().max().cloned(),
        }
    }

    /// The interval an exact left-shift atom takes from its operands':
    /// `value * 2^count` over the count's DEFINED range `[0, count_bits)`.
    /// A count at or beyond the shifted carrier's width produces no value,
    /// so the interval clamps the authored count interval to the defined
    /// range instead of growing the result past any legal evaluation. The
    /// F8 count obligation is judged separately; this interval only covers
    /// evaluations that produce one.
    pub(super) fn shift_left_interval(
        &self,
        value: &Polynomial,
        count: &Polynomial,
        count_bits: u32,
    ) -> Interval {
        let value_interval = self.polynomial_interval(&self.substituted(value));
        let count_interval = self.polynomial_interval(&self.substituted(count));
        let maximum_count = BigInt::from_u64(u64::from(count_bits - 1));
        let low_count = count_interval
            .low
            .unwrap_or_else(BigInt::zero)
            .max(BigInt::zero());
        let high_count = count_interval
            .high
            .unwrap_or_else(|| maximum_count.clone())
            .min(maximum_count);
        if low_count > high_count {
            return Interval::unbounded();
        }
        // Both ends sit in [0, count_bits): the `u64` reads cannot fail.
        let (Some(low_bits), Some(high_bits)) = (low_count.to_u64(), high_count.to_u64()) else {
            return Interval::unbounded();
        };
        value_interval.multiply(&Interval {
            low: Some(BigInt::from_i64(1).shl_bits(low_bits as usize)),
            high: Some(BigInt::from_i64(1).shl_bits(high_bits as usize)),
        })
    }

    /// The interval an exact right-shift atom takes from its operands':
    /// `floor(value / 2^count)` over the count's DEFINED range
    /// `[0, count_bits)`. Floor division by a positive power of two is
    /// monotone in the shifted value and monotone in the count for each
    /// fixed value (falling for a nonnegative value, rising for a negative
    /// one), so the rectangle's extrema sit at its corners. The F8 count
    /// obligation is judged separately; this interval only covers
    /// evaluations that produce one, and such a result never escapes the
    /// shifted carrier.
    pub(super) fn shift_right_interval(
        &self,
        value: &Polynomial,
        count: &Polynomial,
        count_bits: u32,
    ) -> Interval {
        let value_interval = self.polynomial_interval(&self.substituted(value));
        let count_interval = self.polynomial_interval(&self.substituted(count));
        let maximum_count = BigInt::from_u64(u64::from(count_bits - 1));
        let low_count = count_interval
            .low
            .unwrap_or_else(BigInt::zero)
            .max(BigInt::zero());
        let high_count = count_interval
            .high
            .unwrap_or_else(|| maximum_count.clone())
            .min(maximum_count);
        if low_count > high_count {
            return Interval::unbounded();
        }
        // Both ends sit in [0, count_bits): the `u64` reads cannot fail.
        let (Some(low_bits), Some(high_bits)) = (low_count.to_u64(), high_count.to_u64()) else {
            return Interval::unbounded();
        };
        // An unbounded value end keeps its side unbounded: for any fixed
        // count the result still extends to that infinity. A corner that
        // cannot be evaluated fails closed the same way.
        let low = value_interval.low.as_ref().and_then(|bound| {
            floor_shift_right(bound, low_bits)
                .zip(floor_shift_right(bound, high_bits))
                .map(|(low_count, high_count)| low_count.min(high_count))
        });
        let high = value_interval.high.as_ref().and_then(|bound| {
            floor_shift_right(bound, low_bits)
                .zip(floor_shift_right(bound, high_bits))
                .map(|(low_count, high_count)| low_count.max(high_count))
        });
        Interval { low, high }
    }

    pub(super) fn integer_remainder(
        &mut self,
        operand: Polynomial,
        modulus: Polynomial,
        tight_interval: bool,
    ) -> Option<Polynomial> {
        if modulus
            .constant_value()
            .is_some_and(|value| value.is_zero())
        {
            return None;
        }
        let interval = self.remainder_interval(&operand, &modulus, tight_interval);
        // Match quotient identity: normalized operands, not source spelling.
        // The private prefix cannot collide with an authored parameter name.
        let atom = format!("\0integer-remainder:{operand:?}%{modulus:?}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::Remainder {
                operand,
                modulus,
                tight_interval,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    /// Normalize an independently admitted truncating integer quotient. The
    /// caller owns selected operator meaning and each operand/result's carrier
    /// formation; this mathematical term supplies neither of those judgments.
    /// Retain both operands even before nonzero/formation hypotheses are live.
    /// In particular, `0 / divisor` must not erase a still-unproved operation.
    pub(super) fn integer_quotient(
        &mut self,
        dividend: Polynomial,
        divisor: Polynomial,
    ) -> Option<Polynomial> {
        if divisor
            .constant_value()
            .is_some_and(|value| value.is_zero())
        {
            return None;
        }
        if let (Some(dividend), Some(divisor)) =
            (dividend.constant_value(), divisor.constant_value())
        {
            return Some(Polynomial::constant(dividend.div_rem(&divisor)?.0));
        }
        let interval = self.quotient_interval(&dividend, &divisor);
        let atom = format!("\0integer-quotient:{dividend:?}/{divisor:?}");
        self.register_opaque_term(atom.clone(), OpaqueTerm::Quotient { dividend, divisor });
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    /// Normalize an independently admitted exact integer left shift. The
    /// caller owns selected meaning and the F8 count/representability
    /// obligations; this mathematical term supplies neither. A spelled
    /// constant count outside `[0, count_bits)` never produces a value --
    /// like a literal zero divisor, refuse the term rather than minting an
    /// atom that no evaluation can reach.
    pub(super) fn integer_shift_left(
        &mut self,
        value: Polynomial,
        count: Polynomial,
        count_bits: u32,
    ) -> Option<Polynomial> {
        if let Some(count) = count.constant_value() {
            let in_range = count
                .to_u64()
                .is_some_and(|bits| bits < u64::from(count_bits));
            if !in_range {
                return None;
            }
            if let Some(value) = value.constant_value() {
                return Some(Polynomial::constant(
                    value.shl_bits(count.to_u64()? as usize),
                ));
            }
        }
        let interval = self.shift_left_interval(&value, &count, count_bits);
        let atom = format!("\0integer-shift-left:{value:?}<<{count:?}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::ShiftLeft {
                value,
                count,
                count_bits,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    /// Normalize an independently admitted exact integer right shift under
    /// the same contract as [`Self::integer_shift_left`]: the caller owns
    /// selected meaning and the F8 count obligation, and a spelled constant
    /// count outside `[0, count_bits)` never produces a value. Unlike `<<`,
    /// the floor-division result always lands inside the shifted carrier,
    /// so no separate representability question attaches to the term.
    pub(super) fn integer_shift_right(
        &mut self,
        value: Polynomial,
        count: Polynomial,
        count_bits: u32,
    ) -> Option<Polynomial> {
        if let Some(count) = count.constant_value() {
            let in_range = count
                .to_u64()
                .is_some_and(|bits| bits < u64::from(count_bits));
            if !in_range {
                return None;
            }
            if let Some(value) = value.constant_value() {
                return floor_shift_right(&value, count.to_u64()?).map(Polynomial::constant);
            }
        }
        let interval = self.shift_right_interval(&value, &count, count_bits);
        let atom = format!("\0integer-shift-right:{value:?}>>{count:?}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::ShiftRight {
                value,
                count,
                count_bits,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    /// Normalize an independently admitted integer bitwise-AND. The caller
    /// owns selected meaning, the shared primitive's agreement, and each
    /// operand's landing; `a & b` is total inside the `bits`-wide carrier
    /// (two's complement when `signed`), so the term's mathematical value
    /// needs no side condition. Retain both operands: a runtime mask is an
    /// independently transported value too.
    pub(super) fn integer_bitwise_and(
        &mut self,
        left: Polynomial,
        right: Polynomial,
        bits: u32,
        signed: bool,
    ) -> Option<Polynomial> {
        if let (Some(left), Some(right)) = (left.constant_value(), right.constant_value()) {
            return bitwise_and_constants(&left, &right, bits, signed).map(Polynomial::constant);
        }
        let interval = self.bitwise_and_interval(&left, &right, bits, signed);
        // Match quotient identity: normalized operands, not source spelling.
        let atom = format!("\0integer-bitwise-and:{left:?}&{right:?}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::BitwiseAnd {
                left,
                right,
                bits,
                signed,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    /// The interval an integer bitwise-AND atom takes from its operands'
    /// and the shared carrier's representable range. `a & b` is total, so
    /// the carrier's own range bounds every evaluation; each live sign
    /// quadrant then tightens it: a provably nonnegative operand bounds
    /// the result below itself and above zero (its unset sign bit clears
    /// the result's and its set bits cap the magnitude), while two
    /// provably negative operands keep the result below `min(a, b)` inside
    /// the carrier floor. Quadrants the operand intervals exclude
    /// contribute nothing.
    pub(super) fn bitwise_and_interval(
        &self,
        left: &Polynomial,
        right: &Polynomial,
        bits: u32,
        signed: bool,
    ) -> Interval {
        let (minimum, maximum) = bitwise_carrier_bounds(bits, signed);
        let left_interval = self.polynomial_interval(&self.substituted(left));
        let right_interval = self.polynomial_interval(&self.substituted(right));
        // An operand can be nonnegative unless its whole interval sits
        // below zero, and negative unless it never leaves zero.
        let nonnegative =
            |interval: &Interval| !interval.high.as_ref().is_some_and(BigInt::is_negative);
        let negative =
            |interval: &Interval| !interval.low.as_ref().is_some_and(|low| !low.is_negative());
        let min_bound = |left: &Option<BigInt>, right: &Option<BigInt>| match (left, right) {
            (Some(left), Some(right)) => Some(left.clone().min(right.clone())),
            (Some(bound), None) | (None, Some(bound)) => Some(bound.clone()),
            (None, None) => None,
        };
        // The union of the live quadrants: lows min, highs max, and one
        // quadrant without a ceiling lifts the whole result's. `fired`
        // keeps the first bounded quadrant's `Some` from reading as
        // "unbounded so far" -- the union of bounds is empty only before
        // the first live quadrant contributes one.
        let mut low: Option<BigInt> = None;
        let mut high: Option<BigInt> = None;
        let mut fired = false;
        let mut extend = |quadrant_low: BigInt, quadrant_high: Option<BigInt>| {
            low = Some(match &low {
                Some(current) => current.clone().min(quadrant_low.clone()),
                None => quadrant_low,
            });
            high = match quadrant_high {
                Some(bound) if !fired => Some(bound),
                Some(bound) => high.clone().map(|current| current.max(bound)),
                None => None,
            };
            fired = true;
        };
        let (left_nonnegative, left_negative) =
            (nonnegative(&left_interval), negative(&left_interval));
        let (right_nonnegative, right_negative) =
            (nonnegative(&right_interval), negative(&right_interval));
        if left_nonnegative && right_nonnegative {
            extend(
                BigInt::zero(),
                min_bound(&left_interval.high, &right_interval.high),
            );
        }
        if left_nonnegative && right_negative {
            extend(BigInt::zero(), left_interval.high.clone());
        }
        if left_negative && right_nonnegative {
            extend(BigInt::zero(), right_interval.high.clone());
        }
        if left_negative && right_negative {
            extend(
                minimum.clone(),
                min_bound(&left_interval.high, &right_interval.high),
            );
        }
        Interval {
            low: low.map(|low| low.max(minimum.clone())).or(Some(minimum)),
            high: high.map(|high| high.min(maximum.clone())).or(Some(maximum)),
        }
    }
}
