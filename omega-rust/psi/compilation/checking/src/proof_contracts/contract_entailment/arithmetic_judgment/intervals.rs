//! Interval bookkeeping for the entailment engine's opaque integer terms:
//! which range a remainder, quotient, shift, or AND lands inside. The interval
//! type itself is the domain's BigInt carrier; the rules below are the
//! operator-aware judgments on top of it.

use super::super::BigInt;
use super::{BinaryOperator, Engine, OpaqueTerm, Polynomial};
use crate::proof_contracts::arithmetic_domains::{
    BigIntInterval as Interval, bitwise_carrier_bounds, floor_shift_right,
};

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
        // Negating both operands preserves the quotient and makes a strictly
        // negative denominator positive. A zero-inclusive divisor establishes
        // no bound and never obtains definedness from the desired result.
        let (dividend, divisor) = if divisor.high.as_ref().is_some_and(BigInt::is_negative) {
            let negative_one = BigInt::from_i64(-1);
            (dividend.scale(&negative_one), divisor.scale(&negative_one))
        } else {
            (dividend.clone(), divisor.clone())
        };
        if !divisor
            .low
            .as_ref()
            .is_some_and(|low| low > &BigInt::zero())
        {
            return Interval::unbounded();
        }
        // For a positive denominator, each numerator endpoint independently
        // bounds the quotient. Choose its extremal denominator by sign; an
        // infinite denominator contributes the limit zero. An absent numerator
        // endpoint stays unbounded without erasing the other endpoint.
        let quotient_bound = |numerator: &Option<BigInt>, lower: bool| {
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
        Interval {
            low: quotient_bound(&dividend.low, true),
            high: quotient_bound(&dividend.high, false),
        }
    }

    /// The interval an exact left-shift atom takes from its operands':
    /// `value * 2^count` over the count's DEFINED range `[0, count_bits)`.
    /// A count at or beyond the shifted carrier's width produces no value,
    /// so the interval clamps the authored count interval to the defined
    /// range instead of growing the result past any legal evaluation. The
    /// F8 count obligation is judged separately; this interval only covers
    /// evaluations that produce one.
    /// The value interval and the count's `u64`-readable `[0, count_bits)`
    /// bounds a shift interval is computed from; `None` when the count's
    /// range leaves the defined domain or cannot be read.
    fn shift_count_bits(
        &self,
        value: &Polynomial,
        count: &Polynomial,
        count_bits: u32,
    ) -> Option<(Interval, u64, u64)> {
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
            return None;
        }
        // Both ends sit in [0, count_bits): the `u64` reads cannot fail.
        let (Some(low_bits), Some(high_bits)) = (low_count.to_u64(), high_count.to_u64()) else {
            return None;
        };
        Some((value_interval, low_bits, high_bits))
    }

    pub(super) fn shift_left_interval(
        &self,
        value: &Polynomial,
        count: &Polynomial,
        count_bits: u32,
    ) -> Interval {
        let Some((value_interval, low_bits, high_bits)) =
            self.shift_count_bits(value, count, count_bits)
        else {
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
        let Some((value_interval, low_bits, high_bits)) =
            self.shift_count_bits(value, count, count_bits)
        else {
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
        self.integer_shift(
            value,
            count,
            count_bits,
            |value, count| Some(value.shl_bits(count as usize)),
            Self::shift_left_interval,
            "left",
            "<<",
            |value, count, count_bits| OpaqueTerm::ShiftLeft {
                value,
                count,
                count_bits,
            },
        )
    }

    /// The shared exact-integer shift normalization behind
    /// [`Self::integer_shift_left`] and [`Self::integer_shift_right`]:
    /// a spelled constant count outside `[0, count_bits)` refuses the term,
    /// constant operands evaluate eagerly, and opaque terms carry an
    /// interval over the admitted count range.
    #[allow(clippy::too_many_arguments)]
    fn integer_shift(
        &mut self,
        value: Polynomial,
        count: Polynomial,
        count_bits: u32,
        constant: impl Fn(&BigInt, u64) -> Option<BigInt>,
        interval: fn(&Self, &Polynomial, &Polynomial, u32) -> Interval,
        direction: &str,
        operator: &str,
        term: impl Fn(Polynomial, Polynomial, u32) -> OpaqueTerm,
    ) -> Option<Polynomial> {
        if let Some(count) = count.constant_value() {
            let in_range = count
                .to_u64()
                .is_some_and(|bits| bits < u64::from(count_bits));
            if !in_range {
                return None;
            }
            if let Some(value) = value.constant_value() {
                return constant(&value, count.to_u64()?).map(Polynomial::constant);
            }
        }
        let interval = interval(self, &value, &count, count_bits);
        let atom = format!("\0integer-shift-{direction}:{value:?}{operator}{count:?}");
        self.register_opaque_term(atom.clone(), term(value, count, count_bits));
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
        self.integer_shift(
            value,
            count,
            count_bits,
            floor_shift_right,
            Self::shift_right_interval,
            "right",
            ">>",
            |value, count, count_bits| OpaqueTerm::ShiftRight {
                value,
                count,
                count_bits,
            },
        )
    }

    /// Exact known bits compose only through admitted fixed-carrier bitwise
    /// terms. This proves masked observations without assigning values to
    /// unknown bits, enumerating intervals' endpoints, or discarding operand
    /// formation. Carrier identity participates in every opaque term key.
    pub(super) fn integer_bitwise(
        &mut self,
        operator: BinaryOperator,
        left: Polynomial,
        right: Polynomial,
        bits: u32,
        signed: bool,
    ) -> Option<Polynomial> {
        let mask = Self::carrier_mask(bits)?;
        let left = self.substituted(&left);
        let right = self.substituted(&right);
        if left == right {
            return match operator {
                BinaryOperator::BitwiseAnd | BinaryOperator::BitwiseOr => Some(left),
                BinaryOperator::BitwiseXor => Some(Polynomial::constant(BigInt::zero())),
                _ => None,
            };
        }
        let (lz, lo) = self.known_integer_bits(&left, bits, signed, 0);
        let (rz, ro) = self.known_integer_bits(&right, bits, signed, 0);
        let (zero, one) = Self::combine_known_bits(operator, (lz, lo), (rz, ro))?;
        if zero | one == mask {
            let value = BigInt::from_u64(one);
            let value = if signed && one & (1 << (bits - 1)) != 0 {
                value.sub(&BigInt::from_u64(mask).add(&BigInt::from_i64(1)))
            } else {
                value
            };
            return Some(Polynomial::constant(value));
        }
        let interval = self.bitwise_interval(operator, &left, &right, bits, signed);
        let atom = format!("\0integer-bitwise:{operator:?}:{bits}:{signed}:{left:?}:{right:?}");
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::Bitwise {
                operator,
                left,
                right,
                bits,
                signed,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    fn carrier_mask(bits: u32) -> Option<u64> {
        match bits {
            1..=63 => Some((1u64 << bits) - 1),
            64 => Some(u64::MAX),
            _ => None,
        }
    }

    fn combine_known_bits(
        operator: BinaryOperator,
        (az, ao): (u64, u64),
        (bz, bo): (u64, u64),
    ) -> Option<(u64, u64)> {
        Some(match operator {
            BinaryOperator::BitwiseAnd => (az | bz, ao & bo),
            BinaryOperator::BitwiseOr => (az & bz, ao | bo),
            BinaryOperator::BitwiseXor => ((az & bz) | (ao & bo), (az & bo) | (ao & bz)),
            _ => return None,
        })
    }

    fn known_integer_bits(
        &self,
        value: &Polynomial,
        bits: u32,
        signed: bool,
        depth: usize,
    ) -> (u64, u64) {
        let Some(mask) = Self::carrier_mask(bits) else {
            return (0, 0);
        };
        if depth >= 64 {
            return (0, 0);
        }
        let value = self.substituted(value);
        if let Some(value) = value.constant_value() {
            let (minimum, maximum) = bitwise_carrier_bounds(bits, signed);
            if value < minimum || value > maximum {
                return (0, 0);
            }
            let value = if value < BigInt::zero() {
                value.add(&BigInt::from_u64(mask).add(&BigInt::from_i64(1)))
            } else {
                value
            };
            return value
                .to_u64()
                .map(|value| ((!value) & mask, value & mask))
                .unwrap_or((0, 0));
        }
        let Some((atom, 1, constant)) = value.as_single_atom() else {
            return (0, 0);
        };
        if !constant.is_zero() {
            return (0, 0);
        }
        let Some((
            _,
            OpaqueTerm::Bitwise {
                operator,
                left,
                right,
                bits: term_bits,
                signed: term_signed,
            },
        )) = self.opaque_terms.iter().find(|(name, _)| *name == atom)
        else {
            return (0, 0);
        };
        if *term_bits != bits || *term_signed != signed {
            return (0, 0);
        }
        Self::combine_known_bits(
            *operator,
            self.known_integer_bits(left, bits, signed, depth + 1),
            self.known_integer_bits(right, bits, signed, depth + 1),
        )
        .unwrap_or((0, 0))
    }

    pub(super) fn bitwise_interval(
        &self,
        operator: BinaryOperator,
        left: &Polynomial,
        right: &Polynomial,
        bits: u32,
        signed: bool,
    ) -> Interval {
        if operator == BinaryOperator::BitwiseAnd {
            self.bitwise_and_interval(left, right, bits, signed)
        } else {
            let (minimum, maximum) = bitwise_carrier_bounds(bits, signed);
            Interval {
                low: Some(minimum),
                high: Some(maximum),
            }
        }
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
