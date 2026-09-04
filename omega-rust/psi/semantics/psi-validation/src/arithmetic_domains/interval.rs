//! Checked interval lattice and arithmetic for total-specification analysis.

/// An integer value range with optional (= unbounded) ends; all arithmetic is
/// checked, so an overflowing corner becomes `None` (unbounded) -- which fails
/// the containment test and so is reported as a possible overflow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Interval {
    pub(crate) low: Option<i64>,
    pub(crate) high: Option<i64>,
}

impl Interval {
    pub(super) const UNBOUNDED: Interval = Interval {
        low: None,
        high: None,
    };

    pub(crate) fn low(&self) -> Option<i64> {
        self.low
    }

    pub(crate) fn high(&self) -> Option<i64> {
        self.high
    }

    pub(super) fn constant(value: i64) -> Self {
        Self {
            low: Some(value),
            high: Some(value),
        }
    }

    /// Whether every value in this interval is nonzero. A one-sided bound is
    /// sufficient even when the other end exceeds this i64-backed engine (for
    /// example a `u64` known to be at least one).
    pub(super) fn excludes_zero(self) -> bool {
        self.low.is_some_and(|low| low >= 1) || self.high.is_some_and(|high| high <= -1)
    }

    pub(super) fn is_exactly_zero(self) -> bool {
        self.low == Some(0) && self.high == Some(0)
    }

    pub(super) fn add(self, other: Self) -> Self {
        Self {
            low: pair(self.low, other.low, i64::checked_add),
            high: pair(self.high, other.high, i64::checked_add),
        }
    }

    pub(super) fn subtract(self, other: Self) -> Self {
        // [a,b] - [c,d] = [a-d, b-c]
        Self {
            low: pair(self.low, other.high, i64::checked_sub),
            high: pair(self.high, other.low, i64::checked_sub),
        }
    }

    pub(super) fn multiply(self, other: Self) -> Self {
        let (Some(a), Some(b), Some(c), Some(d)) = (self.low, self.high, other.low, other.high)
        else {
            return Interval::UNBOUNDED;
        };
        let corners = [
            a.checked_mul(c),
            a.checked_mul(d),
            b.checked_mul(c),
            b.checked_mul(d),
        ];
        if corners.iter().any(Option::is_none) {
            return Interval::UNBOUNDED;
        }
        let values: Vec<i64> = corners.into_iter().flatten().collect();
        Self {
            low: values.iter().min().copied(),
            high: values.iter().max().copied(),
        }
    }

    /// Mathematical `value * 2^count` bounds for an Exact left shift. A finite,
    /// nonnegative count interval and finite value interval are required. The
    /// four endpoint products cover both monotone sign regions; doing the
    /// arithmetic in `i128` preserves the one representable 64-bit corner
    /// `-1 << 63 == i64::MIN` while still failing closed when any other corner
    /// exceeds this interval engine's representable range.
    pub(super) fn shift_left(self, count: Self) -> Self {
        let (Some(value_low), Some(value_high), Some(count_low), Some(count_high)) =
            (self.low, self.high, count.low, count.high)
        else {
            return Interval::UNBOUNDED;
        };
        let (Ok(count_low), Ok(count_high)) = (u32::try_from(count_low), u32::try_from(count_high))
        else {
            return Interval::UNBOUNDED;
        };
        let (Some(low_factor), Some(high_factor)) = (
            1_i128.checked_shl(count_low),
            1_i128.checked_shl(count_high),
        ) else {
            return Interval::UNBOUNDED;
        };
        let corners = [
            i128::from(value_low).checked_mul(low_factor),
            i128::from(value_low).checked_mul(high_factor),
            i128::from(value_high).checked_mul(low_factor),
            i128::from(value_high).checked_mul(high_factor),
        ];
        if corners.iter().any(Option::is_none) {
            return Interval::UNBOUNDED;
        }
        let values = corners.into_iter().flatten().collect::<Vec<_>>();
        let low = *values.iter().min().expect("four exact shift corners exist");
        let high = *values.iter().max().expect("four exact shift corners exist");
        let (Ok(low), Ok(high)) = (i64::try_from(low), i64::try_from(high)) else {
            return Interval::UNBOUNDED;
        };
        Self {
            low: Some(low),
            high: Some(high),
        }
    }

    pub(super) fn shift_right(self, count: Self) -> Self {
        let (Some(value_low), Some(value_high), Some(count_low), Some(count_high)) =
            (self.low, self.high, count.low, count.high)
        else {
            return Interval::UNBOUNDED;
        };
        let (Ok(count_low), Ok(count_high)) = (u32::try_from(count_low), u32::try_from(count_high))
        else {
            return Interval::UNBOUNDED;
        };
        if count_high >= i64::BITS {
            return Interval::UNBOUNDED;
        }
        let values = [
            value_low >> count_low,
            value_low >> count_high,
            value_high >> count_low,
            value_high >> count_high,
        ];
        Self {
            low: values.iter().min().copied(),
            high: values.iter().max().copied(),
        }
    }

    /// `a % b`: the remainder's magnitude is strictly below the divisor's
    /// magnitude (truncated-division semantics: the remainder takes the
    /// dividend's sign). SOUND only when the divisor is provably nonzero with a
    /// finite magnitude bound; otherwise unbounded (as before). This is what lets
    /// `self.seed % 100` feed exact arithmetic (`% 100` is in `[-99, 99]`)
    /// instead of poisoning the enclosing op with an unbounded operand. The
    /// result interval can only SHRINK relative to the old unbounded value, so it
    /// is strictly permissive for any enclosing overflow check (never a new
    /// rejection).
    pub(super) fn modulo(self, divisor: Self) -> Self {
        let Some(magnitude) = divisor.nonzero_magnitude_bound() else {
            return Interval::UNBOUNDED;
        };
        let bound = magnitude.saturating_sub(1);
        // Remainder sign follows the dividend: a provably non-negative dividend
        // yields a non-negative remainder, a non-positive one a non-positive
        // remainder, an unknown-sign dividend either sign.
        let low = if self.low.is_some_and(|low| low >= 0) {
            0
        } else {
            -bound
        };
        let high = if self.high.is_some_and(|high| high <= 0) {
            0
        } else {
            bound
        };
        Self {
            low: Some(low),
            high: Some(high),
        }
    }

    /// `a / b`: truncated division by a divisor of magnitude >= 1 never grows the
    /// dividend's magnitude and preserves its sign, so the quotient stays within
    /// the dividend's own bounds widened to include 0 (the quotient can reach 0,
    /// e.g. `small / large`). SOUND only when the divisor is provably nonzero;
    /// else unbounded. Like `modulo`, the result can only shrink, so it is
    /// strictly permissive.
    pub(super) fn divide(self, divisor: Self) -> Self {
        if divisor.nonzero_magnitude_bound().is_none() {
            return Interval::UNBOUNDED;
        }
        let (Some(low), Some(high)) = (self.low, self.high) else {
            return Interval::UNBOUNDED;
        };
        // EXACT quotient bounds for a single-valued POSITIVE divisor
        // (`x / 10`): truncated division is monotone non-decreasing in the
        // dividend for k > 0, so `[lo/k, hi/k]` is tight -- `[0, 99] / 10 =
        // [0, 9]`, which is what lets `let tens: u32 [0..=9] = x / 10`
        // store-prove (the range-containment keystone). Any other divisor
        // shape keeps the magnitude-preserving over-approximation.
        if let (Some(k), Some(k_high)) = (divisor.low, divisor.high)
            && k == k_high
            && k > 0
        {
            return Self {
                low: Some(low / k),
                high: Some(high / k),
            };
        }
        Self {
            low: Some(low.min(0)),
            high: Some(high.max(0)),
        }
    }

    /// `min(a, b)`: the result is <= both operands and >= the smaller of the two
    /// possible values. Unbounded ends behave as the appropriate infinity (a
    /// `None` low is -inf, a `None` high is +inf), so `min(x, 100)` upper-bounds
    /// at 100 even when `x` is unbounded.
    pub(super) fn min_with(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
        }
    }

    /// `max(a, b)`: the dual of `min_with` -- `max(0, x)` lower-bounds at 0 even
    /// when `x` is unbounded.
    pub(super) fn max_with(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            },
        }
    }

    /// `Some(max|b|)` when this interval (a divisor) is finite and provably
    /// excludes 0 -- either entirely positive (`low >= 1`) or entirely negative
    /// (`high <= -1`). `None` otherwise (the divisor may be 0 or is unbounded, so
    /// `%`/`/` cannot be magnitude-bounded soundly).
    pub(super) fn nonzero_magnitude_bound(self) -> Option<i64> {
        let (low, high) = (self.low?, self.high?);
        if low >= 1 || high <= -1 {
            Some(low.saturating_abs().max(high.saturating_abs()))
        } else {
            None
        }
    }

    /// Widest interval covering both (`[min(lows), max(highs)]`, an unbounded end
    /// on EITHER side making that side unbounded). Used to union a callee's
    /// multiple return paths when inferring its return range.
    pub(super) fn union(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.min(b)),
                _ => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.max(b)),
                _ => None,
            },
        }
    }

    /// Tightest interval contained in both (`[max(lows), min(highs)]`, an
    /// unbounded end deferring to the other). Used to intersect a guard bound
    /// with a place's type range.
    pub(super) fn intersect(self, other: Self) -> Self {
        Self {
            low: match (self.low, other.low) {
                (Some(a), Some(b)) => Some(a.max(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
            high: match (self.high, other.high) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(v), None) | (None, Some(v)) => Some(v),
                (None, None) => None,
            },
        }
    }

    /// Does `self` (a type's range) fully contain `inner` (a computed value
    /// range)? An unbounded `inner` end against a bounded `self` end is NOT
    /// contained -- the value might exceed the type.
    pub(super) fn contains(self, inner: Interval) -> bool {
        let low_ok = match (self.low, inner.low) {
            (Some(bound), Some(value)) => value >= bound,
            (Some(_), None) => false,
            (None, _) => true,
        };
        let high_ok = match (self.high, inner.high) {
            (Some(bound), Some(value)) => value <= bound,
            (Some(_), None) => false,
            (None, _) => true,
        };
        low_ok && high_ok
    }
}

fn pair(left: Option<i64>, right: Option<i64>, op: fn(i64, i64) -> Option<i64>) -> Option<i64> {
    match (left, right) {
        (Some(a), Some(b)) => op(a, b),
        _ => None,
    }
}
