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
    /// Preserve bounds derived from independently formed operands, rather than
    /// trying to rediscover correlation in an expanded product. This is a
    /// mathematical interval; it does not assume that the result fits a carrier.
    pub(super) fn install_exact_product_interval(
        &mut self,
        left: &Polynomial,
        right: &Polynomial,
        value: &Polynomial,
    ) {
        let interval = self
            .polynomial_interval(&self.substituted(left))
            .multiply(&self.polynomial_interval(&self.substituted(right)));
        for bound in interval
            .low
            .map(|low| (value.clone(), low))
            .into_iter()
            .chain(interval.high.map(|high| (value.neg(), high.negate())))
        {
            if !self.bounds.contains(&bound) {
                self.bounds.push(bound);
            }
        }
        self.refresh_difference_bounds();
    }

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
        operand_interval.remainder(&divisor_interval)
    }

    pub(super) fn constant_remainder_interval(
        &self,
        operand: &Polynomial,
        modulus: &BigInt,
        tight_interval: bool,
    ) -> Interval {
        self.polynomial_interval(&self.substituted(operand))
            .constant_remainder(modulus, tight_interval)
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
        dividend.divide(&Interval::constant(divisor.clone()))
    }

    pub(super) fn bounded_quotient_interval(dividend: &Interval, divisor: &Interval) -> Interval {
        dividend.divide(divisor)
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
        value_interval.shift_right(low_bits, high_bits)
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
        // Every monomial is integral. A nonzero constant dividing every
        // coefficient therefore divides the entire polynomial, for either
        // dividend sign under truncating remainder. Intervals alone forget
        // this correlation (for example, (count * 2) % 2).
        // This simplifies only the value: callers retain the original operand
        // and divisor formation obligations, including Exact MIN % -1 and
        // overflow in the polynomial's authored intermediate operations.
        if let Some(divisor) = modulus.constant_value()
            && operand.terms.values().all(|coefficient| {
                coefficient
                    .div_rem(&divisor)
                    .is_some_and(|(_, remainder)| remainder.is_zero())
            })
        {
            return Some(Polynomial::constant(BigInt::zero()));
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
        // Equal established bounds identify one exact integer even when its
        // contract used inequalities. Wider intervals supply no exact bits:
        // bitwise results are not monotonic in their endpoints.
        let known_value =
            value
                .constant_value()
                .or_else(|| match self.polynomial_interval(&value) {
                    Interval {
                        low: Some(low),
                        high: Some(high),
                    } if low == high => Some(low),
                    _ => None,
                });
        if let Some(value) = known_value {
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
        self.polynomial_interval(&self.substituted(left))
            .bitwise_and(
                &self.polynomial_interval(&self.substituted(right)),
                bits,
                signed,
            )
    }
}
