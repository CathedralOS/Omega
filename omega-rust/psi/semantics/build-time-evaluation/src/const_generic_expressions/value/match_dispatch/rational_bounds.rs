//! All-arm numeric evidence for anonymous arithmetic containing dispatch.
//!
//! A union of closed rational intervals contains every result, regardless of
//! which arm executes. Negative and positive alternatives stay separate at
//! joins, preserving their exclusion of zero through surrounding arithmetic.
//! At most one hull is retained in each of three categories: wholly negative,
//! wholly positive, or containing zero. A zero-containing range is never split
//! or discarded without an independent lattice exclusion. Each binary has at
//! most nine interval pairs, independent of the number of branch combinations.
//! No branch-selection state is retained.
//!
//! Joins still lose correlations and gaps not captured by the lattice. A possible
//! zero leaves an obligation open; it is not evidence of an executed zero divisor.
//! A rational lattice additionally contains every result as offset + stride*k
//! for some integer k. Joins retain a common divisor of strides and offsets;
//! arithmetic transports it without rounding intermediate fractions. Integral
//! offset and stride prove integrality, while the intervals prove carrier fit.
//! The same lattice can exclude zero inside an interval hull: split that hull
//! at the nearest lattice points on either side of zero. This intersection of
//! two overapproximations retains every result without storing individual arms.
//! It also gives division zero-free intervals on which corner bounds are valid;
//! a nonzero Boolean alone would not justify dividing across a continuous pole.
//! An integral pair of interval endpoints alone proves neither: joining 1, 1.5,
//! and 2 must not erase the fractional interior. Nonconstant division generally
//! loses lattice evidence, even when its rational bounds remain useful.
//! Integer interval analysis has different division and width semantics, so only
//! its interval laws apply here; all arithmetic uses the shared exact rationals.
//!
//! The caller has validated the complete acyclic scalar graph and checks every
//! subject, pattern, and arm. This pass visits only anonymous result edges; it
//! cannot execute landed operations in an undemanded subject or select an arm.
//! Each binary composes two bounded summaries, not authored branch selections.
//! Both children are checked even when multiplication by zero would erase their
//! result, since an undefined anonymous subexpression has no numeric value.

use numerics::bignum::{BigInt, BigRational};
use numerics::literals::LandedIntegerType;
use typed_trees::{
    TypedTrees,
    expression::{BinaryOperator, ExpressionHandle, ExpressionNode},
};
use validation::evaluate_anonymous_numeric_expression_with_selected_match_arms;

pub(super) fn excludes_zero(
    program: &TypedTrees,
    root: ExpressionHandle,
    builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Result<bool, String> {
    Ok(analyze(program, root, builtin)?.excludes_zero())
}

pub(super) fn validate_integer_landing(
    program: &TypedTrees,
    root: ExpressionHandle,
    carrier: LandedIntegerType,
    builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Result<bool, String> {
    let bounds = analyze(program, root, builtin)?;
    if !bounds
        .lattice
        .as_ref()
        .is_some_and(RationalLattice::is_integral)
    {
        return Err(
            "anonymous constant Match landing requires an all-arm integral result proof".into(),
        );
    }
    let width = carrier.bit_width();
    let (minimum, maximum) = if carrier.is_signed() {
        let magnitude = 1i128 << (width - 1);
        (-magnitude, magnitude - 1)
    } else {
        (0, (1i128 << width) - 1)
    };
    let minimum = BigRational::from_integer(BigInt::from_i128(minimum));
    let maximum = BigRational::from_integer(BigInt::from_i128(maximum));
    if !bounds.intervals().all(|interval| {
        !interval.low.cmp_value(&minimum).is_lt() && !interval.high.cmp_value(&maximum).is_gt()
    }) {
        return Err(format!(
            "anonymous constant Match landing requires every arm to fit `{}`",
            carrier.name()
        ));
    }
    Ok(bounds.fractional_history)
}

fn analyze(
    program: &TypedTrees,
    root: ExpressionHandle,
    mut builtin: impl FnMut(ExpressionHandle) -> bool,
) -> Result<RationalBounds, String> {
    enum Step {
        Enter(ExpressionHandle),
        Binary(BinaryOperator),
        Join(usize),
    }
    let mut pending = vec![Step::Enter(root)];
    let mut values: Vec<RationalBounds> = Vec::new();
    while let Some(step) = pending.pop() {
        match step {
            Step::Enter(expression) => match program.expression_table.expression(expression) {
                ExpressionNode::Match(dispatch) => {
                    let arms = program.expression_table.match_arms(dispatch.arms);
                    if arms.is_empty() || arms.len() != dispatch.arms.len() {
                        return Err("anonymous rational bounds require valid Match arms".into());
                    }
                    pending.push(Step::Join(arms.len()));
                    pending.extend(arms.iter().rev().map(|arm| Step::Enter(arm.value)));
                }
                ExpressionNode::Binary(binary) => {
                    if !builtin(expression) {
                        return Err(
                            "anonymous rational bounds require selected builtin meaning".into()
                        );
                    }
                    pending.push(Step::Binary(binary.operator));
                    pending.push(Step::Enter(binary.right));
                    pending.push(Step::Enter(binary.left));
                }
                ExpressionNode::Integer(_) | ExpressionNode::Float(_) => {
                    let value = evaluate_anonymous_numeric_expression_with_selected_match_arms(
                        program,
                        expression,
                        &[],
                        &mut builtin,
                    )
                    .ok_or("anonymous rational bounds require exact anonymous literals")?;
                    values.push(RationalBounds::constant(value));
                }
                _ => return Err("unsupported expression in anonymous rational bounds".into()),
            },
            Step::Binary(operator) => {
                let right = values
                    .pop()
                    .ok_or("missing right anonymous rational bounds")?;
                let left = values
                    .pop()
                    .ok_or("missing left anonymous rational bounds")?;
                values.push(left.apply(operator, &right)?);
            }
            Step::Join(count) => {
                let mut joined = values
                    .pop()
                    .ok_or("missing anonymous Match result bounds")?;
                for _ in 1..count {
                    joined.include(values.pop().ok_or("missing anonymous Match arm bounds")?);
                }
                joined.refine_zero_gap();
                values.push(joined);
            }
        }
    }
    if values.len() != 1 {
        return Err("anonymous arithmetic did not produce one rational range".into());
    }
    values
        .pop()
        .ok_or("missing anonymous rational bounds".into())
}

#[derive(Default)]
struct RationalBounds {
    negative: Option<RationalInterval>,
    positive: Option<RationalInterval>,
    containing_zero: Option<RationalInterval>,
    lattice: Option<RationalLattice>,
    fractional_history: bool,
}

impl RationalBounds {
    fn constant(value: BigRational) -> Self {
        let mut bounds = Self {
            fractional_history: value.to_integer_exact().is_none(),
            lattice: Some(RationalLattice {
                offset: value.clone(),
                stride: BigRational::zero(),
            }),
            ..Self::default()
        };
        bounds.include_interval(RationalInterval::constant(value));
        bounds
    }

    fn excludes_zero(&self) -> bool {
        self.containing_zero.is_none() && (self.negative.is_some() || self.positive.is_some())
    }

    fn intervals(&self) -> impl Iterator<Item = &RationalInterval> {
        self.negative
            .iter()
            .chain(&self.positive)
            .chain(&self.containing_zero)
    }

    fn include_interval(&mut self, interval: RationalInterval) {
        let destination = if interval.high.cmp_value(&BigRational::zero()).is_lt() {
            &mut self.negative
        } else if interval.low.cmp_value(&BigRational::zero()).is_gt() {
            &mut self.positive
        } else {
            &mut self.containing_zero
        };
        if let Some(existing) = destination {
            existing.include(interval);
        } else {
            *destination = Some(interval);
        }
    }

    fn include(&mut self, other: Self) {
        self.fractional_history |= other.fractional_history;
        self.lattice = self
            .lattice
            .as_ref()
            .zip(other.lattice.as_ref())
            .and_then(|(left, right)| left.join(right));
        for interval in [other.negative, other.positive, other.containing_zero]
            .into_iter()
            .flatten()
        {
            self.include_interval(interval);
        }
    }

    fn refine_zero_gap(&mut self) {
        if self.containing_zero.is_none() {
            return;
        }
        let Some([negative, positive]) = self
            .lattice
            .as_ref()
            .and_then(RationalLattice::zero_neighbors)
        else {
            return;
        };
        let Some(interval) = self.containing_zero.take() else {
            return;
        };
        // Only the open lattice gap is removed. Clip to the original bounds:
        // extending an endpoint would discard the independent carrier evidence.
        if !interval.low.cmp_value(&negative).is_gt() {
            self.include_interval(RationalInterval {
                low: interval.low,
                high: negative,
            });
        }
        if !interval.high.cmp_value(&positive).is_lt() {
            self.include_interval(RationalInterval {
                low: positive,
                high: interval.high,
            });
        }
    }

    fn apply(&self, operator: BinaryOperator, right: &Self) -> Result<Self, String> {
        let mut result = Self {
            lattice: self
                .lattice
                .as_ref()
                .zip(right.lattice.as_ref())
                .and_then(|(left, right)| left.apply(operator, right)),
            ..Self::default()
        };
        result.fractional_history = self.fractional_history
            || right.fractional_history
            || !result
                .lattice
                .as_ref()
                .is_some_and(RationalLattice::is_integral);
        for left_interval in self.intervals() {
            for right_interval in right.intervals() {
                result.include_interval(left_interval.apply(operator, right_interval)?);
            }
        }
        result.refine_zero_gap();
        if result.intervals().next().is_none() {
            return Err("anonymous rational arithmetic requires nonempty bounds".into());
        }
        Ok(result)
    }
}

struct RationalLattice {
    offset: BigRational,
    stride: BigRational,
}

impl RationalLattice {
    fn is_integral(&self) -> bool {
        self.offset.to_integer_exact().is_some() && self.stride.to_integer_exact().is_some()
    }

    /// Nearest negative/positive points, only when zero is absent. The residue
    /// of offset/|stride| identifies the same lattice for either stride sign or
    /// any choice of offset representative. Truncated negative remainders need
    /// Euclidean correction; no fixed-width conversion or rounding is involved.
    fn zero_neighbors(&self) -> Option<[BigRational; 2]> {
        let stride = if self.stride.is_negative() {
            self.stride.negate()
        } else {
            self.stride.clone()
        };
        let coordinate = self.offset.div(&stride)?;
        let (numerator, denominator) = coordinate.as_integer_ratio();
        let (_, mut remainder) = numerator.div_rem(denominator)?;
        if remainder.is_zero() {
            return None;
        }
        if remainder.is_negative() {
            remainder = remainder.add(denominator);
        }
        let residue = BigRational::from_integer(remainder)
            .div(&BigRational::from_integer(denominator.clone()))?;
        let positive = stride.mul(&residue);
        Some([positive.sub(&stride), positive])
    }

    fn join(&self, other: &Self) -> Option<Self> {
        Some(Self {
            offset: self.offset.clone(),
            stride: common_divisor(
                &common_divisor(&self.stride, &other.stride)?,
                &self.offset.sub(&other.offset),
            )?,
        })
    }

    fn apply(&self, operator: BinaryOperator, right: &Self) -> Option<Self> {
        let (offset, stride) = match operator {
            BinaryOperator::Add | BinaryOperator::Subtract => (
                if operator == BinaryOperator::Add {
                    self.offset.add(&right.offset)
                } else {
                    self.offset.sub(&right.offset)
                },
                common_divisor(&self.stride, &right.stride)?,
            ),
            // (a + s*m)(b + t*n) = ab + sb*m + at*n + st*m*n.
            BinaryOperator::Multiply => (
                self.offset.mul(&right.offset),
                common_divisor(
                    &common_divisor(
                        &self.stride.mul(&right.offset),
                        &self.offset.mul(&right.stride),
                    )?,
                    &self.stride.mul(&right.stride),
                )?,
            ),
            BinaryOperator::Divide if right.stride.is_zero() => (
                self.offset.div(&right.offset)?,
                self.stride.div(&right.offset)?,
            ),
            _ => return None,
        };
        Some(Self { offset, stride })
    }
}

/// On a shared positive denominator, integer gcd gives the rational unit of
/// which both operands are integer multiples. Zero denotes a singleton stride.
fn common_divisor(left: &BigRational, right: &BigRational) -> Option<BigRational> {
    let (left_numerator, left_denominator) = left.as_integer_ratio();
    let (right_numerator, right_denominator) = right.as_integer_ratio();
    let numerator = left_numerator
        .mul(right_denominator)
        .gcd(&right_numerator.mul(left_denominator));
    let denominator = left_denominator.mul(right_denominator);
    BigRational::from_integer(numerator).div(&BigRational::from_integer(denominator))
}

struct RationalInterval {
    low: BigRational,
    high: BigRational,
}

impl RationalInterval {
    fn constant(value: BigRational) -> Self {
        Self {
            low: value.clone(),
            high: value,
        }
    }

    fn excludes_zero(&self) -> bool {
        self.low.cmp_value(&BigRational::zero()).is_gt()
            || self.high.cmp_value(&BigRational::zero()).is_lt()
    }

    fn include(&mut self, other: Self) {
        if other.low.cmp_value(&self.low).is_lt() {
            self.low = other.low;
        }
        if other.high.cmp_value(&self.high).is_gt() {
            self.high = other.high;
        }
    }

    fn corners([first, second, third, fourth]: [BigRational; 4]) -> Self {
        let mut bounds = Self::constant(first);
        for value in [second, third, fourth] {
            bounds.include(Self::constant(value));
        }
        bounds
    }

    fn apply(&self, operator: BinaryOperator, right: &Self) -> Result<Self, String> {
        Ok(match operator {
            BinaryOperator::Add => Self {
                low: self.low.add(&right.low),
                high: self.high.add(&right.high),
            },
            BinaryOperator::Subtract => Self {
                low: self.low.sub(&right.high),
                high: self.high.sub(&right.low),
            },
            BinaryOperator::Multiply => Self::corners([
                self.low.mul(&right.low),
                self.low.mul(&right.high),
                self.high.mul(&right.low),
                self.high.mul(&right.high),
            ]),
            BinaryOperator::Divide => {
                if !right.excludes_zero() {
                    return Err("anonymous rational division requires a nonzero divisor proof; its rational bounds include zero".into());
                }
                // On a zero-free denominator interval the quotient extrema
                // occur at corners, also for negative or fractional endpoints.
                let divide = |left: &BigRational, right: &BigRational| {
                    left.div(right)
                        .ok_or("undefined anonymous rational quotient")
                };
                Self::corners([
                    divide(&self.low, &right.low)?,
                    divide(&self.low, &right.high)?,
                    divide(&self.high, &right.low)?,
                    divide(&self.high, &right.high)?,
                ])
            }
            _ => return Err("anonymous rational bounds require arithmetic meaning".into()),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use numerics::bignum::BigInt;

    fn fraction(numerator: i64, denominator: i64) -> BigRational {
        BigRational::from_integer(BigInt::from_i64(numerator))
            .div(&BigRational::from_integer(BigInt::from_i64(denominator)))
            .expect("nonzero test denominator")
    }

    fn lattice_contains(lattice: &RationalLattice, value: &BigRational) -> bool {
        let difference = value.sub(&lattice.offset);
        if lattice.stride.is_zero() {
            difference.is_zero()
        } else {
            difference
                .div(&lattice.stride)
                .is_some_and(|multiple| multiple.to_integer_exact().is_some())
        }
    }

    #[test]
    fn lattice_zero_neighbors_preserve_exact_rational_spacing() {
        for offset_numerator in -7..=7 {
            for stride_numerator in -5..=5 {
                let lattice = RationalLattice {
                    offset: fraction(offset_numerator, 3),
                    stride: fraction(stride_numerator, 2),
                };
                let Some([negative, positive]) = lattice.zero_neighbors() else {
                    assert!(
                        lattice.stride.is_zero()
                            || lattice_contains(&lattice, &BigRational::zero())
                    );
                    continue;
                };
                assert!(!lattice_contains(&lattice, &BigRational::zero()));
                assert!(negative.cmp_value(&BigRational::zero()).is_lt());
                assert!(positive.cmp_value(&BigRational::zero()).is_gt());
                assert!(lattice_contains(&lattice, &negative));
                assert!(lattice_contains(&lattice, &positive));
                let spacing = if lattice.stride.is_negative() {
                    lattice.stride.negate()
                } else {
                    lattice.stride.clone()
                };
                assert_eq!(positive.sub(&negative), spacing);
            }
        }
    }

    #[test]
    fn lattice_gap_refinement_retains_every_sampled_point_in_original_bounds() {
        for offset_numerator in -4..=4 {
            for stride_numerator in -3..=3 {
                for low_numerator in -3..=0 {
                    for high_numerator in 0..=3 {
                        let lattice = RationalLattice {
                            offset: fraction(offset_numerator, 3),
                            stride: fraction(stride_numerator, 2),
                        };
                        let low = fraction(low_numerator, 2);
                        let high = fraction(high_numerator, 2);
                        let mut points = Vec::new();
                        for multiple in -12..=12 {
                            let point = lattice
                                .offset
                                .add(&lattice.stride.mul(&fraction(multiple, 1)));
                            if !point.cmp_value(&low).is_lt() && !point.cmp_value(&high).is_gt() {
                                points.push(point);
                            }
                        }
                        let mut bounds = RationalBounds {
                            containing_zero: Some(RationalInterval {
                                low: low.clone(),
                                high: high.clone(),
                            }),
                            lattice: Some(lattice),
                            fractional_history: true,
                            ..RationalBounds::default()
                        };
                        bounds.refine_zero_gap();
                        assert!(bounds.intervals().count() <= 3);
                        assert!(bounds.fractional_history);
                        for interval in bounds.intervals() {
                            assert!(!interval.low.cmp_value(&low).is_lt());
                            assert!(!interval.high.cmp_value(&high).is_gt());
                            assert!(!interval.low.cmp_value(&interval.high).is_gt());
                        }
                        for point in points {
                            assert!(
                                bounds
                                    .intervals()
                                    .any(|interval| !point.cmp_value(&interval.low).is_lt()
                                        && !point.cmp_value(&interval.high).is_gt())
                            );
                            if point.is_zero() {
                                assert!(!bounds.excludes_zero());
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn joining_a_zero_arm_restores_the_nonzero_obligation() {
        for values in [[1, 3], [3, 1]] {
            let mut bounds = RationalBounds::constant(fraction(values[0], 1));
            bounds.include(RationalBounds::constant(fraction(values[1], 1)));
            let mut difference = bounds
                .apply(
                    BinaryOperator::Subtract,
                    &RationalBounds::constant(fraction(2, 1)),
                )
                .expect("defined difference");
            assert!(difference.excludes_zero(), "arithmetic split the zero gap");
            difference.include(RationalBounds::constant(fraction(0, 1)));
            difference.refine_zero_gap();
            assert!(!difference.excludes_zero());
            assert!(
                RationalBounds::constant(fraction(1, 1))
                    .apply(BinaryOperator::Divide, &difference)
                    .is_err()
            );
        }
    }

    #[test]
    fn rational_lattice_operations_contain_sampled_exact_values() {
        for left_offset in -2..=2 {
            for right_offset in -2..=2 {
                for left_stride in 0..=2 {
                    for right_stride in 0..=2 {
                        let left = RationalLattice {
                            offset: fraction(left_offset, 2),
                            stride: fraction(left_stride, 2),
                        };
                        let right = RationalLattice {
                            offset: fraction(right_offset, 3),
                            stride: fraction(right_stride, 3),
                        };
                        let joined = left.join(&right).expect("rational gcd");
                        for operator in [
                            BinaryOperator::Add,
                            BinaryOperator::Subtract,
                            BinaryOperator::Multiply,
                            BinaryOperator::Divide,
                        ] {
                            let Some(result) = left.apply(operator, &right) else {
                                assert_eq!(operator, BinaryOperator::Divide);
                                assert!(right_stride != 0 || right_offset == 0);
                                continue;
                            };
                            for left_multiple in -2..=2 {
                                let left_value = left
                                    .offset
                                    .add(&left.stride.mul(&fraction(left_multiple, 1)));
                                assert!(lattice_contains(&joined, &left_value));
                                for right_multiple in -2..=2 {
                                    let right_value = right
                                        .offset
                                        .add(&right.stride.mul(&fraction(right_multiple, 1)));
                                    assert!(lattice_contains(&joined, &right_value));
                                    let value = match operator {
                                        BinaryOperator::Add => left_value.add(&right_value),
                                        BinaryOperator::Subtract => left_value.sub(&right_value),
                                        BinaryOperator::Multiply => left_value.mul(&right_value),
                                        BinaryOperator::Divide => left_value
                                            .div(&right_value)
                                            .expect("fixed nonzero divisor"),
                                        _ => unreachable!(),
                                    };
                                    assert!(
                                        lattice_contains(&result, &value),
                                        "{operator:?}: {value:?}"
                                    );
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn integer_endpoints_do_not_prove_fractional_interiors_integral() {
        let mut bounds = RationalBounds::constant(fraction(1, 1));
        bounds.include(RationalBounds::constant(fraction(3, 2)));
        bounds.include(RationalBounds::constant(fraction(2, 1)));
        assert!(
            bounds
                .intervals()
                .all(|interval| interval.low.to_integer_exact().is_some()
                    && interval.high.to_integer_exact().is_some())
        );
        assert!(
            !bounds
                .lattice
                .as_ref()
                .expect("rational lattice")
                .is_integral()
        );
        let doubled = bounds
            .apply(
                BinaryOperator::Multiply,
                &RationalBounds::constant(fraction(2, 1)),
            )
            .expect("defined arithmetic");
        assert!(
            doubled
                .lattice
                .as_ref()
                .expect("transported lattice")
                .is_integral()
        );
        assert!(
            doubled.fractional_history,
            "cancellation cannot erase diagnostic history"
        );

        let mut divisors = RationalBounds::constant(fraction(1, 1));
        divisors.include(RationalBounds::constant(fraction(6, 1)));
        let quotients = RationalBounds::constant(fraction(6, 1))
            .apply(BinaryOperator::Divide, &divisors)
            .expect("nonzero interval");
        assert!(
            quotients
                .intervals()
                .all(|interval| interval.low.to_integer_exact().is_some()
                    && interval.high.to_integer_exact().is_some())
        );
        assert!(
            quotients.lattice.is_none(),
            "6/4 is fractional inside integer extrema 1..6"
        );
    }

    #[test]
    fn rational_bounds_contain_interior_and_endpoint_arithmetic() {
        for left_low in -2..=2 {
            for left_high in left_low..=2 {
                for right_low in -2..=2 {
                    for right_high in right_low..=2 {
                        for operator in [
                            BinaryOperator::Add,
                            BinaryOperator::Subtract,
                            BinaryOperator::Multiply,
                            BinaryOperator::Divide,
                        ] {
                            let left = RationalInterval {
                                low: fraction(left_low, 2),
                                high: fraction(left_high, 2),
                            };
                            let right = RationalInterval {
                                low: fraction(right_low, 2),
                                high: fraction(right_high, 2),
                            };
                            let result = left.apply(operator, &right);
                            if operator == BinaryOperator::Divide
                                && right_low <= 0
                                && right_high >= 0
                            {
                                assert!(result.is_err(), "zero-containing denominator interval");
                                continue;
                            }
                            let bounds = result.expect("defined rational bounds");
                            for left in [
                                fraction(left_low, 2),
                                fraction(left_low + left_high, 4),
                                fraction(left_high, 2),
                            ] {
                                for right in [
                                    fraction(right_low, 2),
                                    fraction(right_low + right_high, 4),
                                    fraction(right_high, 2),
                                ] {
                                    let actual = match operator {
                                        BinaryOperator::Add => left.add(&right),
                                        BinaryOperator::Subtract => left.sub(&right),
                                        BinaryOperator::Multiply => left.mul(&right),
                                        BinaryOperator::Divide => {
                                            left.div(&right).expect("zero-free interval")
                                        }
                                        _ => unreachable!(),
                                    };
                                    assert!(!bounds.low.cmp_value(&actual).is_gt());
                                    assert!(!bounds.high.cmp_value(&actual).is_lt());
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn sign_partitioned_bounds_preserve_all_category_pairs() {
        fn bounds(categories: u8) -> RationalBounds {
            let mut result = RationalBounds::default();
            for (category, low, high) in [(1, -3, -1), (2, 1, 3), (4, -1, 1)] {
                if categories & category != 0 {
                    result.include_interval(RationalInterval {
                        low: fraction(low, 2),
                        high: fraction(high, 2),
                    });
                }
            }
            result
        }

        let empty = RationalBounds::default();
        assert!(!empty.excludes_zero());
        assert!(empty.apply(BinaryOperator::Add, &bounds(1)).is_err());
        assert!(bounds(1).apply(BinaryOperator::Add, &empty).is_err());
        for left_categories in 1..8 {
            for right_categories in 1..8 {
                let left = bounds(left_categories);
                let right = bounds(right_categories);
                for operator in [
                    BinaryOperator::Add,
                    BinaryOperator::Subtract,
                    BinaryOperator::Multiply,
                    BinaryOperator::Divide,
                ] {
                    let result = left.apply(operator, &right);
                    if operator == BinaryOperator::Divide && right_categories & 4 != 0 {
                        assert!(
                            result.is_err(),
                            "zero-containing alternatives cannot be discarded"
                        );
                        continue;
                    }
                    let result = result.expect("defined category arithmetic");
                    assert!(result.intervals().count() <= 3);
                    for left in left.intervals() {
                        for right in right.intervals() {
                            for left in [&left.low, &left.high] {
                                for right in [&right.low, &right.high] {
                                    let actual = match operator {
                                        BinaryOperator::Add => left.add(right),
                                        BinaryOperator::Subtract => left.sub(right),
                                        BinaryOperator::Multiply => left.mul(right),
                                        BinaryOperator::Divide => {
                                            left.div(right).expect("zero-free denominator")
                                        }
                                        _ => unreachable!(),
                                    };
                                    assert!(
                                        result.intervals().any(|range| !range
                                            .low
                                            .cmp_value(&actual)
                                            .is_gt()
                                            && !range.high.cmp_value(&actual).is_lt())
                                    );
                                    if actual.is_zero() {
                                        assert!(!result.excludes_zero());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn rational_bounds_visit_result_operations_once_without_subject_execution() {
        use source_files_to_tokens::Lexer;
        use typed_trees::statement::StatementNode;

        for (term, operation, expected_operator) in [
            (
                "(match (1u8 / 0 == 0) { true -> 1, false -> 2 })",
                " + ",
                BinaryOperator::Add,
            ),
            (
                "(match (1u8 / 0 == 0) { true -> -1, false -> 1 })",
                " * ",
                BinaryOperator::Multiply,
            ),
        ] {
            let expression = std::iter::repeat_n(term, 24)
                .collect::<Vec<_>>()
                .join(operation);
            let source = format!("machine choose() -> u8 {{ {expression} }}");
            let syntax = tokens_to_syntax_trees::parse_syntax_trees(
                &Lexer::new(&source).tokenize().expect("tokens"),
            )
            .expect("syntax");
            let resolved = syntax_trees_to_symbol_resolved_trees::lower_syntax_trees(&syntax)
                .expect("resolved");
            let program =
                symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees(&resolved)
                    .expect("typed");
            let machine = &program.machines()[0];
            let state = &program.machine_states(machine)[0];
            let StatementNode::Expression(root) =
                program.statement_table.statements(state.statement_nodes)[0]
            else {
                panic!("expression fixture");
            };
            super::super::validate_graph(&program, root).expect("validated caller precondition");
            let mut visits = 0;
            assert!(excludes_zero(&program, root, |expression| {
            visits += 1;
            assert!(matches!(program.expression_table.expression(expression), ExpressionNode::Binary(binary) if binary.operator == expected_operator));
            validation::has_builtin_binary_expression_meaning(&program, machine, Some(state), expression)
        }).expect("zero-free sum or product"));
            assert_eq!(
                visits, 23,
                "one visit per operation, no branch combinations or subject operations"
            );
            assert!(
                excludes_zero(&program, root, |_| false).is_err(),
                "token spelling grants no builtin authority"
            );
        }
    }
}
