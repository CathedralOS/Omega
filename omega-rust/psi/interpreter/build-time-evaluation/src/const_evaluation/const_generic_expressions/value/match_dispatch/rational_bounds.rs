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
//! While the summarized value set stays small, the bounds also carry it
//! exactly: a sorted point list unions at arm joins and composes pairwise at
//! each binary, then drops away past a fixed size. The list only ever
//! overapproximates the reachable values — joins merge no phantom points and
//! binaries add none — so it discharges proofs the hulls cannot: a joined
//! lattice that reaches zero still yields to a zero-free point list, and a
//! divisor cell whose interval spans the pole still divides when its exact
//! points are all nonzero. Lattice admissibility, by contrast, can name
//! points no arm produces ({2, 3} multiplied by {2, 4} admits 10 inside
//! [4, 12] on 4+2Z), so exact divisors keep integrality proofs the
//! lattice enumeration must decline. The point list is flat across sign
//! cells; each division clips it to one cell's interval to recover that
//! cell's own admissible divisors. Points are operand values, never branch
//! selections: two dispatch operands stay independent because the pairwise
//! composition unions every cross pair.
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

#[derive(Clone, Default)]
struct RationalBounds {
    negative: Option<RationalCell>,
    positive: Option<RationalCell>,
    containing_zero: Option<RationalCell>,
    fractional_history: bool,
    /// The exact admissible values while the set stays bounded: a sorted
    /// deduplicated list covering every reachable result and nothing the
    /// operand lists could not produce. `None` falls back to the hull cells.
    points: Option<Vec<BigRational>>,
}

/// One sign-category hull: a closed interval paired with the lattice covering
/// exactly the values summarized there. The lattice is evidence over this
/// hull's values alone, so composing a cell pair never dilutes a sibling
/// hull's retained gap.
#[derive(Clone)]
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
            points: Some(vec![value.clone()]),
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
        // Exact points answer the pole question directly; the hull cells only
        // decide when the point list was dropped.
        if let Some(points) = &self.points {
            return !points.is_empty() && points.iter().all(|point| !point.is_zero());
        }
        self.containing_zero.is_none() && (self.negative.is_some() || self.positive.is_some())
    }

    /// Every summarized value is provably integral: at least one hull exists
    /// and each hull's effective evidence lands on integers. A collapsed
    /// singleton carries its own point as that evidence. The exact point
    /// list answers directly when retained.
    fn has_integral_lattice(&self) -> bool {
        if let Some(points) = &self.points {
            return !points.is_empty()
                && points
                    .iter()
                    .all(|point| point.to_integer_exact().is_some());
        }
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
        self.points = match (self.points.take(), other.points) {
            (Some(mut left), Some(right)) => {
                left.extend(right);
                sorted_unique(&mut left);
                (left.len() <= MAX_EXACT_POINTS).then_some(left)
            }
            _ => None,
        };
        for cell in [other.negative, other.positive, other.containing_zero]
            .into_iter()
            .flatten()
        {
            self.include_cell(cell.interval, cell.lattice);
        }
    }

    /// The admissible divisor values of one cell, when finitely known: the
    /// exact bound points clipped to the cell's interval, else the cell
    /// lattice's own points inside it. An empty list marks a vacuous cell
    /// whose contribution cannot occur; `None` declines an enumeration no
    /// finite evidence covers.
    fn admissible_divisors(&self, cell: &RationalCell) -> Option<Vec<BigRational>> {
        if let Some(points) = &self.points {
            return Some(
                points
                    .iter()
                    .filter(|point| cell.interval.contains(point))
                    .cloned()
                    .collect(),
            );
        }
        cell.evidence()?.points_within(&cell.interval)
    }

    /// One operand cell pair's contribution to a quotient bound. Each exact
    /// admissible divisor produces its own piece — a corner interval clipped
    /// to that divisor and the quotient lattice it induces — so pieces keep
    /// their own gaps instead of joining one diluted hull. A divisor cell
    /// whose interval reaches zero can still divide when the bound's exact
    /// point list names every admissible value and none is zero; lattice
    /// enumeration alone cannot split a continuous pole. An empty admissible
    /// set is vacuous and contributes only its interval hull; an
    /// unenumerable set keeps the corner interval without lattice evidence.
    fn include_quotient(
        &mut self,
        left: &RationalCell,
        right: &RationalBounds,
        right_cell: &RationalCell,
    ) -> Result<(), String> {
        let divisors = right.admissible_divisors(right_cell);
        if !right_cell.interval.excludes_zero() {
            match &divisors {
                Some(divisors) if divisors.iter().all(|divisor| !divisor.is_zero()) => {}
                _ => {
                    return Err("anonymous rational division requires a nonzero divisor proof; its rational bounds include zero".into());
                }
            }
        }
        match divisors {
            Some(divisors) if !divisors.is_empty() => {
                for divisor in divisors {
                    let low = left
                        .interval
                        .low
                        .div(&divisor)
                        .ok_or("undefined anonymous rational quotient")?;
                    let high = left
                        .interval
                        .high
                        .div(&divisor)
                        .ok_or("undefined anonymous rational quotient")?;
                    let interval = if low.cmp_value(&high).is_gt() {
                        RationalInterval {
                            low: high,
                            high: low,
                        }
                    } else {
                        RationalInterval { low, high }
                    };
                    let lattice = left.evidence().and_then(|left| {
                        Some(RationalLattice {
                            offset: left.offset.div(&divisor)?,
                            stride: left.stride.div(&divisor)?,
                        })
                    });
                    self.include_cell(interval, lattice);
                }
            }
            _ => {
                let interval = left
                    .interval
                    .apply(BinaryOperator::Divide, &right_cell.interval)?;
                self.include_cell(interval, None);
            }
        }
        Ok(())
    }

    fn apply(&self, operator: BinaryOperator, right: &Self) -> Result<Self, String> {
        let mut result = Self {
            fractional_history: self.fractional_history || right.fractional_history,
            points: exact_apply(operator, &self.points, &right.points),
            ..Self::default()
        };
        // Each result hull's lattice joins only the operand cell pairs whose
        // intervals land there, so a pair routing into another sign category
        // cannot erase this hull's gap. A pair without lattice evidence
        // leaves its result hull lattice-free even when endpoints would
        // divide evenly.
        for left_cell in self.cells() {
            for right_cell in right.cells() {
                if operator == BinaryOperator::Divide {
                    result.include_quotient(left_cell, right, right_cell)?;
                    continue;
                }
                let interval = left_cell.interval.apply(operator, &right_cell.interval)?;
                let lattice = left_cell
                    .evidence()
                    .zip(right_cell.evidence())
                    .and_then(|(left, right)| left.apply(operator, &right));
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

/// The pairwise composition of two exact point lists, while the work stays
/// bounded. Division by a listed zero still declines here; the hull pass
/// reports the pole through its own proof. The result count is capped after
/// deduplication, so independent dispatches grow the list by distinct values
/// rather than by branch combinations.
fn exact_apply(
    operator: BinaryOperator,
    left: &Option<Vec<BigRational>>,
    right: &Option<Vec<BigRational>>,
) -> Option<Vec<BigRational>> {
    let (left, right) = left.as_ref().zip(right.as_ref())?;
    if left.len().checked_mul(right.len())? > MAX_EXACT_POINT_PRODUCTS {
        return None;
    }
    let mut points = Vec::new();
    for left in left {
        for right in right {
            let value = match operator {
                BinaryOperator::Add => left.add(right),
                BinaryOperator::Subtract => left.sub(right),
                BinaryOperator::Multiply => left.mul(right),
                BinaryOperator::Divide => left.div(right)?,
                _ => return None,
            };
            points.push(value);
        }
    }
    sorted_unique(&mut points);
    (points.len() <= MAX_EXACT_POINTS).then_some(points)
}

fn sorted_unique(points: &mut Vec<BigRational>) {
    points.sort_by(|left, right| left.cmp_value(right));
    points.dedup_by(|left, right| left.cmp_value(right).is_eq());
}

/// The retained exact point list's bounds: at most this many distinct values
/// survive a join or composition, and pairwise composition is only attempted
/// below this product. Past either bound the hull cells carry the proof.
const MAX_EXACT_POINTS: usize = 256;
const MAX_EXACT_POINT_PRODUCTS: usize = 4096;

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

#[derive(Clone)]
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

    fn contains(&self, point: &BigRational) -> bool {
        !point.cmp_value(&self.low).is_lt() && !point.cmp_value(&self.high).is_gt()
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
