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
