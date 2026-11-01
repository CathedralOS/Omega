//! The contract-entailment oracle's arbitrary-precision integer interval,
//! hosted in the interval domain beside the `i64`/`i128` carriers: literal-heavy
//! judgments need ends that can leave machine width, so this carrier keeps
//! `Option<BigInt>` ends (`None` = unbounded). Operator-aware rules that mint
//! opaque terms stay with the entailment engine.

use numerics::bignum::BigInt;

/// An interval with optional (= unbounded) ends; end arithmetic is exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BigIntInterval {
    pub low: Option<BigInt>,
    pub high: Option<BigInt>,
}

impl BigIntInterval {
    pub fn unbounded() -> Self {
        Self {
            low: None,
            high: None,
        }
    }

    pub fn constant(value: BigInt) -> Self {
        Self {
            low: Some(value.clone()),
            high: Some(value),
        }
    }

    /// Tightest interval contained in both. An absent end is unbounded and
    /// defers to the other side, the same lattice meet as `Interval::intersect`.
    /// A finite end outside `i64` stays exact: it is not clamped to a machine
    /// extreme.
    pub fn intersect(&self, other: &Self) -> Self {
        Self {
            low: match (&self.low, &other.low) {
                (Some(left), Some(right)) => Some(left.clone().max(right.clone())),
                (Some(value), None) | (None, Some(value)) => Some(value.clone()),
                (None, None) => None,
            },
            high: match (&self.high, &other.high) {
                (Some(left), Some(right)) => Some(left.clone().min(right.clone())),
                (Some(value), None) | (None, Some(value)) => Some(value.clone()),
                (None, None) => None,
            },
        }
    }

    pub fn add(&self, other: &Self) -> Self {
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

    pub fn subtract(&self, other: &Self) -> Self {
        Self {
            low: self
                .low
                .as_ref()
                .zip(other.high.as_ref())
                .map(|(left, right)| left.sub(right)),
            high: self
                .high
                .as_ref()
                .zip(other.low.as_ref())
                .map(|(left, right)| left.sub(right)),
        }
    }

    pub fn union(&self, other: &Self) -> Self {
        Self {
            low: self
                .low
                .as_ref()
                .zip(other.low.as_ref())
                .map(|(left, right)| left.clone().min(right.clone())),
            high: self
                .high
                .as_ref()
                .zip(other.high.as_ref())
                .map(|(left, right)| left.clone().max(right.clone())),
        }
    }

    pub fn excludes_zero(&self) -> bool {
        self.low.as_ref().is_some_and(|low| low > &BigInt::zero())
            || self.high.as_ref().is_some_and(BigInt::is_negative)
    }

    /// Bounds of truncating division on a nonzero divisor interval. Infinite
    /// denominator ends contribute the limit zero; numerator ends remain
    /// independent, so one unbounded end cannot erase the other finite end.
    /// This supplies result bounds, never the caller's formation judgment.
    pub fn divide(&self, divisor: &Self) -> Self {
        let (dividend, divisor) = if divisor.high.as_ref().is_some_and(BigInt::is_negative) {
            let negative_one = BigInt::from_i64(-1);
            (self.scale(&negative_one), divisor.scale(&negative_one))
        } else {
            (self.clone(), divisor.clone())
        };
        if !divisor
            .low
            .as_ref()
            .is_some_and(|low| low > &BigInt::zero())
        {
            return Self::unbounded();
        }
        let endpoint = |numerator: &Option<BigInt>, lower: bool| {
            let numerator = numerator.as_ref()?;
            let denominator = if lower == numerator.is_negative() {
                &divisor.low
            } else {
                &divisor.high
            };
            match denominator {
                Some(denominator) => numerator.div_rem(denominator).map(|(quotient, _)| quotient),
                None => Some(BigInt::zero()),
            }
        };
        Self {
            low: endpoint(&dividend.low, true),
            high: endpoint(&dividend.high, false),
        }
    }

    /// The hull of defined truncating remainders. A result can be zero even
    /// when the dividend cannot; signed minimum magnitudes stay arbitrary
    /// precision. Definedness and fixed-width MIN / -1 remain separate.
    pub fn remainder(&self, divisor: &Self) -> Self {
        let dividend = self.union(&Self::constant(BigInt::zero()));
        let Some(magnitude) = divisor
            .low
            .as_ref()
            .zip(divisor.high.as_ref())
            .map(|(low, high)| low.abs().max(high.abs()))
        else {
            return dividend;
        };
        if magnitude.is_zero() {
            return Self::unbounded();
        }
        let bound = magnitude.sub(&BigInt::from_i64(1));
        dividend.intersect(&Self {
            low: Some(bound.negate()),
            high: Some(bound),
        })
    }

    /// A caller that admitted the exact remainder reading may retain the
    /// stronger hull when every dividend shares one truncating quotient.
    pub fn constant_remainder(&self, modulus: &BigInt, tight_interval: bool) -> Self {
        if modulus.is_zero() {
            return Self::unbounded();
        }
        if tight_interval
            && let (Some(low), Some(high)) = (&self.low, &self.high)
            && let (Some((low_quotient, low_remainder)), Some((high_quotient, high_remainder))) =
                (low.div_rem(modulus), high.div_rem(modulus))
            && low_quotient == high_quotient
        {
            return Self {
                low: Some(low_remainder.clone().min(high_remainder.clone())),
                high: Some(low_remainder.max(high_remainder)),
            };
        }
        self.remainder(&Self::constant(modulus.clone()))
    }

    /// Bounds of an arithmetic right shift over an independently established
    /// finite count range. Unknown value ends remain unknown on their side.
    pub fn shift_right(&self, low_count: u64, high_count: u64) -> Self {
        if low_count > high_count {
            return Self::unbounded();
        }
        let low = self.low.as_ref().and_then(|value| {
            floor_shift_right(value, low_count)
                .zip(floor_shift_right(value, high_count))
                .map(|(low, high)| low.min(high))
        });
        let high = self.high.as_ref().and_then(|value| {
            floor_shift_right(value, low_count)
                .zip(floor_shift_right(value, high_count))
                .map(|(low, high)| low.max(high))
        });
        Self { low, high }
    }

    /// Conservative carrier-aware AND bounds from the operand sign quadrants.
    /// The selected carrier, not endpoint monotonicity, supplies representation
    /// meaning. Full bit-prefix kernels remain in the shared numeric foundation.
    pub fn bitwise_and(&self, other: &Self, bits: u32, signed: bool) -> Self {
        let (minimum, maximum) = bitwise_carrier_bounds(bits, signed);
        let left_interval = self;
        let right_interval = other;
        // An operand can be nonnegative unless its whole interval sits
        // below zero, and negative unless it never leaves zero.
        let nonnegative =
            |interval: &Self| !interval.high.as_ref().is_some_and(BigInt::is_negative);
        let negative =
            |interval: &Self| !interval.low.as_ref().is_some_and(|low| !low.is_negative());
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
            (nonnegative(left_interval), negative(left_interval));
        let (right_nonnegative, right_negative) =
            (nonnegative(right_interval), negative(right_interval));
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
        Self {
            low: low.map(|low| low.max(minimum.clone())).or(Some(minimum)),
            high: high.map(|high| high.min(maximum.clone())).or(Some(maximum)),
        }
    }

    pub fn from_closed(range: facts::IntegerRange) -> Self {
        Self {
            low: Some(range.minimum),
            high: Some(range.maximum),
        }
    }

    /// Only two established, ordered finite ends can become a closed evidence
    /// row. Unknown endpoints are never clipped to a machine carrier.
    pub fn into_closed(self) -> Option<facts::IntegerRange> {
        let minimum = self.low?;
        let maximum = self.high?;
        (minimum <= maximum).then_some(facts::IntegerRange { minimum, maximum })
    }

    pub fn scale(&self, factor: &BigInt) -> Self {
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

    pub fn multiply(&self, other: &Self) -> Self {
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
            return Self::unbounded();
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
    pub fn correlated_power(&self, power: u32) -> Self {
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
                Self::unbounded()
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

fn pow(base: &BigInt, power: u32) -> BigInt {
    let mut result = BigInt::from_i64(1);
    for _ in 0..power {
        result = result.mul(base);
    }
    result
}

/// `floor(value / 2^count)`: an exact `>>`'s mathematical value. The engine's
/// `div_rem` truncates toward zero, so a negative nondivisible value drops
/// one more -- `-3 >> 1` is `-2`, not the truncated quotient `-1`.
pub fn floor_shift_right(value: &BigInt, count: u64) -> Option<BigInt> {
    if count >= value.bit_length() as u64 {
        return Some(if value.is_negative() {
            BigInt::from_i64(-1)
        } else {
            BigInt::zero()
        });
    }
    let divisor = BigInt::from_i64(1).shl_bits(usize::try_from(count).ok()?);
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
pub fn bitwise_carrier_bounds(bits: u32, signed: bool) -> (BigInt, BigInt) {
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
