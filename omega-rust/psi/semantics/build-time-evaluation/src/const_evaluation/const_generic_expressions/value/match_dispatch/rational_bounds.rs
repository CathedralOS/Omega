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
//! for some integer k. Each hull keeps the lattice covering exactly the values
//! it summarizes, so the retained evidence is the per-hull intersection of two
//! overapproximations rather than one join diluted by every arm and operation.
//! A zero-spanning pair like (-5+4Z)+{4} keeps its own zero-free -1+4Z lattice:
//! each contributing interval is split at its own lattice's nearest points
//! around zero before merging, so a sibling pair's contribution cannot dilute
//! the gap by joining first. Once merged, a zero-containing hull's lattice
//! either reaches zero or is absent, and the pole stays excluded without
//! storing individual arms. The same intersection gives division zero-free
//! intervals on which corner bounds are valid; a nonzero Boolean alone would
//! not justify dividing across a continuous pole. An integral pair of
//! interval endpoints alone proves neither: joining 1, 1.5, and 2 must not
//! erase the fractional interior. Division retains lattice evidence through
//! the admissible divisor values inside each sign interval: the interval's
//! intersection with the divisor's own lattice is a finite set of exact
//! points, so a singleton bound is only the one-point case and a joined
//! same-sign pair like {2, 3} still enumerates its two divisors. A hull whose
//! lattice intersection is wider declines rather than scanning a range no
//! finite evidence covers. A hull collapsed to one point is its own exact
//! evidence: the point needs no retained lattice to prove integrality or to
//! stand as an admissible value. Integer interval analysis has different
//! division and width semantics, so only its interval laws apply here; all
//! arithmetic uses the shared exact rationals.
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
    if !bounds.has_integral_lattice() {
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
    negative: Option<RationalCell>,
    positive: Option<RationalCell>,
    containing_zero: Option<RationalCell>,
    fractional_history: bool,
}

/// One sign-category hull: a closed interval paired with the lattice covering
/// exactly the values summarized there. The lattice is evidence over this
/// hull's values alone, so composing a cell pair never dilutes a sibling
/// hull's retained gap.
struct RationalCell {
    interval: RationalInterval,
    lattice: Option<RationalLattice>,
}

impl RationalCell {
    /// Lattice evidence for this hull: the retained lattice, or the hull's
    /// own point when its interval has collapsed to one exact value. A
    /// singleton needs no retained lattice — it admits only that value — so
    /// the point itself proves integrality or stands as an admissible
    /// operand. Explicit retained evidence always takes precedence.
    fn evidence(&self) -> Option<RationalLattice> {
        self.lattice
            .clone()
            .or_else(|| singleton_lattice(&self.interval))
    }

    fn include(&mut self, interval: RationalInterval, lattice: Option<RationalLattice>) {
        // Both sides merge at their own effective evidence so a collapsed
        // singleton's exact point still joins rather than erasing the hull's
        // retained lattice. The interval widens only after that read.
        self.lattice = self
            .evidence()
            .zip(lattice.or_else(|| singleton_lattice(&interval)))
            .and_then(|(left, right)| left.join(&right));
        self.interval.include(interval);
    }
}

impl RationalBounds {
    fn constant(value: BigRational) -> Self {
        let mut bounds = Self {
            fractional_history: value.to_integer_exact().is_none(),
            ..Self::default()
        };
        bounds.include_cell(
            RationalInterval::constant(value.clone()),
            Some(RationalLattice {
                offset: value,
                stride: BigRational::zero(),
            }),
        );
        bounds
    }

    fn excludes_zero(&self) -> bool {
        self.containing_zero.is_none() && (self.negative.is_some() || self.positive.is_some())
    }

    /// Every summarized value is provably integral: at least one hull exists
    /// and each hull's effective evidence lands on integers. A collapsed
    /// singleton carries its own point as that evidence.
    fn has_integral_lattice(&self) -> bool {
        self.cells().next().is_some()
            && self
                .cells()
                .all(|cell| cell.evidence().is_some_and(|lattice| lattice.is_integral()))
    }

    fn cells(&self) -> impl Iterator<Item = &RationalCell> {
        self.negative
            .iter()
            .chain(&self.positive)
            .chain(&self.containing_zero)
    }

    fn intervals(&self) -> impl Iterator<Item = &RationalInterval> {
        self.cells().map(|cell| &cell.interval)
    }

    /// Fold one interval and its covering lattice into the matching sign
    /// hull. The lattice must cover every value the interval contributes;
    /// without that evidence the merged hull retains interval bounds only. A
    /// singleton interval is its own exact evidence: one known point proves
    /// integrality or an admissible value without a retained lattice. A
    /// zero-spanning contribution whose own lattice excludes zero splits at
    /// the gap before merging, so a sibling contribution cannot join the
    /// merged hull's lattice down to a stride that reaches zero. Each clipped
    /// piece keeps the contribution's own lattice, whose points still cover
    /// the piece's values; a piece outside the retained bounds contributes
    /// nothing. A merged zero-containing hull therefore only ever joins
    /// contributions whose own evidence cannot exclude zero.
    fn include_cell(&mut self, interval: RationalInterval, lattice: Option<RationalLattice>) {
        if !interval.excludes_zero()
            && let Some([negative, positive]) =
                lattice.as_ref().and_then(RationalLattice::zero_neighbors)
        {
            if !interval.low.cmp_value(&negative).is_gt() {
                self.include_cell(
                    RationalInterval {
                        low: interval.low.clone(),
                        high: negative,
                    },
                    lattice.clone(),
                );
            }
            if !interval.high.cmp_value(&positive).is_lt() {
                self.include_cell(
                    RationalInterval {
                        low: positive,
                        high: interval.high,
                    },
                    lattice,
                );
            }
            return;
        }
        let destination = if interval.high.cmp_value(&BigRational::zero()).is_lt() {
            &mut self.negative
        } else if interval.low.cmp_value(&BigRational::zero()).is_gt() {
            &mut self.positive
        } else {
            &mut self.containing_zero
        };
        if let Some(existing) = destination {
            existing.include(interval, lattice);
        } else {
            *destination = Some(RationalCell { interval, lattice });
        }
    }

    fn include(&mut self, other: Self) {
        self.fractional_history |= other.fractional_history;
        for cell in [other.negative, other.positive, other.containing_zero]
            .into_iter()
            .flatten()
        {
            self.include_cell(cell.interval, cell.lattice);
        }
    }

    fn apply(&self, operator: BinaryOperator, right: &Self) -> Result<Self, String> {
        let mut result = Self {
            fractional_history: self.fractional_history || right.fractional_history,
            ..Self::default()
        };
        // Each result hull's lattice joins only the operand cell pairs whose
        // intervals land there, so a pair routing into another sign category
        // cannot erase this hull's gap. A pair without lattice evidence
        // leaves its result hull lattice-free even when endpoints would
        // divide evenly.
        for left_cell in self.cells() {
            for right_cell in right.cells() {
                let interval = left_cell.interval.apply(operator, &right_cell.interval)?;
                let lattice = match operator {
                    BinaryOperator::Divide => divide_lattice(left_cell, right_cell),
                    _ => left_cell
                        .evidence()
                        .zip(right_cell.evidence())
                        .and_then(|(left, right)| left.apply(operator, &right)),
                };
                result.include_cell(interval, lattice);
            }
        }
        let unproven_integrality = result
            .cells()
            .any(|cell| !cell.evidence().is_some_and(|lattice| lattice.is_integral()));
        result.fractional_history |= unproven_integrality;
        if result.intervals().next().is_none() {
            return Err("anonymous rational arithmetic requires nonempty bounds".into());
        }
        Ok(result)
    }
}

/// A collapsed hull's own point is exact evidence: the interval admits only
/// that value, so it proves integrality, an admissible divisor, or a lattice
/// neighbor without a retained lattice. Explicit lattice evidence always
/// takes precedence; this never widens a hull's claimed value set.
fn singleton_lattice(interval: &RationalInterval) -> Option<RationalLattice> {
    (interval.low == interval.high).then(|| RationalLattice {
        offset: interval.low.clone(),
        stride: BigRational::zero(),
    })
}

/// Quotient lattice covering one operand cell pair under division. For x in
/// a+sZ and an exact nonzero d, x/d is in a/d+(s/d)Z. Each right hull's
/// admissible divisors are its intersection with its own lattice: a finite
/// set of exact points, so a joined same-sign hull like [2,3] on 2+Z still
/// contributes {2,3} and a lattice-free hull still needs the hull's single
/// endpoint. Join every quotient lattice; a pair whose admissible set cannot
/// be enumerated forfeits only its own result hull's lattice. This inspects
/// at most three summary cells per operand, never authored arms.
fn divide_lattice(left: &RationalCell, right: &RationalCell) -> Option<RationalLattice> {
    let left = left.evidence()?;
    let divisors = right.evidence()?.points_within(&right.interval)?;
    let mut joined: Option<RationalLattice> = None;
    for divisor in divisors {
        let quotient = RationalLattice {
            offset: left.offset.div(&divisor)?,
            stride: left.stride.div(&divisor)?,
        };
        joined = Some(match joined {
            Some(previous) => previous.join(&quotient)?,
            None => quotient,
        });
    }
    joined
}

#[derive(Clone)]
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

    /// Every lattice point inside one closed interval, which is that
    /// interval's admissible value set when the lattice already bounds it:
    /// real values lie on both overapproximations, so their intersection
    /// keeps the enumeration exact. A constant lattice contributes its
    /// offset when covered. Otherwise the lattice indices are solved against
    /// the endpoints with exact floor and ceiling; a wider solution than the
    /// enumeration bound declines.
    fn points_within(&self, interval: &RationalInterval) -> Option<Vec<BigRational>> {
        if self.stride.is_zero() {
            let covered = !self.offset.cmp_value(&interval.low).is_lt()
                && !self.offset.cmp_value(&interval.high).is_gt();
            return Some(if covered {
                vec![self.offset.clone()]
            } else {
                Vec::new()
            });
        }
        // The lattice is also offset + |stride|*k for integer k, so solving
        // the endpoint indices against the absolute stride enumerates the
        // same points in increasing order.
        let stride = if self.stride.is_negative() {
            self.stride.negate()
        } else {
            self.stride.clone()
        };
        let lowest = ceiling(&interval.low.sub(&self.offset).div(&stride)?);
        let highest = floor(&interval.high.sub(&self.offset).div(&stride)?);
        if highest < lowest {
            return Some(Vec::new());
        }
        let width = highest.sub(&lowest);
        let width = width.to_u64()?;
        if width >= MAX_ENUMERATED_LATTICE_POINTS as u64 {
            return None;
        }
        let mut points = Vec::with_capacity(width as usize + 1);
        let mut point = self
            .offset
            .add(&stride.mul(&BigRational::from_integer(lowest.clone())));
        let mut index = lowest;
        loop {
            points.push(point.clone());
            if index == highest {
                break;
            }
            index = index.add(&BigInt::from_u64(1));
            point = point.add(&stride);
        }
        Some(points)
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

/// The enumeration bound for one hull's admissible lattice points. Each point
/// is an exact value of the bound itself rather than a branch combination,
/// but a wide interval on a fine stride still declines rather than scanning
/// a range no authored bound could distinguish.
const MAX_ENUMERATED_LATTICE_POINTS: usize = 256;

/// The exact floor of a rational: the greatest lattice index whose point
/// still reaches its endpoint. Truncated division already rounds toward
/// zero, so only a negative remainder steps down.
fn floor(value: &BigRational) -> BigInt {
    let (numerator, denominator) = value.as_integer_ratio();
    let (quotient, remainder) = numerator
        .div_rem(denominator)
        .expect("a rational denominator is nonzero");
    if remainder.is_negative() {
        quotient.sub(&BigInt::from_u64(1))
    } else {
        quotient
    }
}

/// The exact ceiling of a rational: the least lattice index whose point
/// reaches its endpoint. Under truncated division only a positive remainder
/// steps up.
fn ceiling(value: &BigRational) -> BigInt {
    let (numerator, denominator) = value.as_integer_ratio();
    let (quotient, remainder) = numerator
        .div_rem(denominator)
        .expect("a rational denominator is nonzero");
    if remainder.is_zero() || remainder.is_negative() {
        quotient
    } else {
        quotient.add(&BigInt::from_u64(1))
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
mod tests;
