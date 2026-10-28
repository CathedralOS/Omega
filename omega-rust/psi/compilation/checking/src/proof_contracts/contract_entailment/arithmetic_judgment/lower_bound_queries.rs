//! Pure lower-bound queries over one immutable arithmetic engine.
//!
//! Final comparison batches prepare canonical exact-bound and atom-interval
//! lookups once. The immutable borrow prevents premise, substitution, matrix or
//! opaque-interval changes until those queries finish. Admission-time callers
//! use direct reads through the same proof bodies, without allocating indexes.
//! Preparation adds no facts. It reuses each strongest canonical bound for
//! residual and scaled queries, whose judgments are pure and threshold-monotone.
use super::{BigInt, Engine, Interval, Polynomial, ZERO_ATOM};
use std::collections::BTreeMap;

#[derive(Clone)]
pub(in crate::proof_contracts::contract_entailment) struct PreparedLowerBoundIndex {
    exact_indices: Vec<u32>,
    intervals: Vec<(String, Interval)>,
    proportional: Vec<u32>,
}

impl PreparedLowerBoundIndex {
    pub(in crate::proof_contracts::contract_entailment) fn new(engine: &Engine<'_>) -> Self {
        let mut exact: Vec<u32> = (0..engine.bounds.len() as u32).collect();
        exact.sort_by(|&left, &right| {
            engine.bounds[left as usize]
                .0
                .terms
                .cmp(&engine.bounds[right as usize].0.terms)
        });
        // Equal polynomials produce the same residual and scale. Their
        // strongest bound gives the weakest requested residual threshold, so
        // it subsumes every weaker copy for all immutable queries below.
        let mut strongest: Vec<u32> = Vec::with_capacity(exact.len());
        for idx in exact {
            let (value, bound) = &engine.bounds[idx as usize];
            if let Some(&previous_idx) = strongest.last()
                && engine.bounds[previous_idx as usize].0 == *value
            {
                if *bound > engine.bounds[previous_idx as usize].1
                    && let Some(last) = strongest.last_mut()
                {
                    *last = idx;
                }
            } else {
                strongest.push(idx);
            }
        }
        // Positive proportionality requires exactly the same nonconstant
        // monomials. Index that necessary condition before comparing coefficients;
        // unrelated captured scalars cannot match a residual merely because they
        // occur in the same immutable premise batch. Keep the existing proof body
        // authoritative within each group, including constants and integer slack.
        let mut proportional: Vec<u32> = (0..strongest.len() as u32).collect();
        proportional.sort_by(|&left, &right| {
            nonconstant_monomials_cmp(
                &engine.bounds[strongest[left as usize] as usize].0,
                &engine.bounds[strongest[right as usize] as usize].0,
            )
        });
        let mut atoms: Vec<&str> = engine
            .parameter_atoms
            .iter()
            .map(String::as_str)
            .chain(engine.unsigned_atoms.iter().map(String::as_str))
            .chain(engine.matrix.atom_names())
            .chain(engine.arithmetic_intervals.keys().map(String::as_str))
            .chain(engine.bounds.iter().flat_map(|(value, _)| {
                value
                    .terms
                    .keys()
                    .flat_map(|monomial| monomial.keys().map(String::as_str))
            }))
            .chain(std::iter::once(ZERO_ATOM))
            .collect();
        atoms.sort_unstable();
        atoms.dedup();
        // Use the original accessor, including arithmetic-interval override,
        // unsigned lower bounds and the matrix's virtual zero diagonal.
        let intervals = atoms
            .into_iter()
            .map(|atom| (atom.to_owned(), engine.atom_interval(atom)))
            .collect();
        Self {
            exact_indices: strongest,
            intervals,
            proportional,
        }
    }
}

enum PreparedBoundsRef<'a> {
    Owned(PreparedLowerBoundIndex),
    Borrowed(&'a PreparedLowerBoundIndex),
}

pub(in crate::proof_contracts::contract_entailment) struct LowerBoundQueries<'engine, 'program> {
    engine: &'engine Engine<'program>,
    prepared: Option<PreparedBoundsRef<'engine>>,
}

impl<'engine, 'program> LowerBoundQueries<'engine, 'program> {
    pub(in crate::proof_contracts::contract_entailment) fn direct(
        engine: &'engine Engine<'program>,
    ) -> Self {
        Self {
            engine,
            prepared: None,
        }
    }

    pub(super) fn prepared(engine: &'engine Engine<'program>) -> Self {
        Self {
            engine,
            prepared: Some(PreparedBoundsRef::Owned(PreparedLowerBoundIndex::new(
                engine,
            ))),
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn from_prepared_index(
        engine: &'engine Engine<'program>,
        index: &'engine PreparedLowerBoundIndex,
    ) -> Self {
        Self {
            engine,
            prepared: Some(PreparedBoundsRef::Borrowed(index)),
        }
    }

    fn prepared_index(&self) -> Option<&PreparedLowerBoundIndex> {
        match &self.prepared {
            Some(PreparedBoundsRef::Owned(index)) => Some(index),
            Some(PreparedBoundsRef::Borrowed(index)) => Some(index),
            None => None,
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn engine(&self) -> &Engine<'program> {
        self.engine
    }

    pub(super) fn prepared_exact_bounds(&self) -> Vec<(&'engine Polynomial, &'engine BigInt)> {
        self.prepared_index().map_or_else(Vec::new, |prepared| {
            prepared
                .exact_indices
                .iter()
                .map(|&idx| {
                    let (poly, bound) = &self.engine.bounds[idx as usize];
                    (poly, bound)
                })
                .collect()
        })
    }

    fn exact_bound_at_least(&self, value: &Polynomial, bound: &BigInt) -> bool {
        if let Some(prepared) = self.prepared_index() {
            prepared
                .exact_indices
                .binary_search_by(|&idx| {
                    self.engine.bounds[idx as usize].0.terms.cmp(&value.terms)
                })
                .is_ok_and(|position| {
                    &self.engine.bounds[prepared.exact_indices[position] as usize].1 >= bound
                })
        } else {
            self.engine
                .bounds
                .iter()
                .any(|(stored, stored_bound)| stored == value && stored_bound >= bound)
        }
    }

    fn atom_interval(&self, atom: &str) -> Interval {
        if let Some(prepared) = self.prepared_index()
            && let Ok(position) = prepared
                .intervals
                .binary_search_by_key(&atom, |(stored, _)| stored.as_str())
        {
            return prepared.intervals[position].1.clone();
        }
        // A queried residual may mention an atom absent from the prepared
        // premise roster. Its original lookup remains authoritative.
        self.engine.atom_interval(atom)
    }

    /// Prove a lower bound directly or by adding one stored lower bound to
    /// a residual established by the existing nonrecursive judgment.
    pub(in crate::proof_contracts::contract_entailment) fn prove_at_least(
        &self,
        polynomial: &Polynomial,
        bound: &BigInt,
    ) -> bool {
        if self.prove_base_lower_bound(polynomial, bound) {
            return true;
        }
        let proves_from_bound = |stored: &Polynomial, stored_bound: &BigInt| {
            // P >= b and goal - P >= requested - b imply goal >= requested.
            // Do not recursively search combinations of stored hypotheses.
            self.prove_base_lower_bound(&polynomial.sub(stored), &bound.sub(stored_bound))
                || self.prove_scaled_lower_bound(polynomial, bound, stored, stored_bound)
        };
        if let Some(prepared) = self.prepared_index() {
            prepared.exact_indices.iter().any(|&idx| {
                let (stored, stored_bound) = &self.engine.bounds[idx as usize];
                proves_from_bound(stored, stored_bound)
            })
        } else {
            self.engine
                .bounds
                .iter()
                .any(|(stored, stored_bound)| proves_from_bound(stored, stored_bound))
        }
    }

    /// Difference bounds, exact hypotheses, and independent intervals only.
    ///
    /// This query is pure: contradiction checking reuses failed queries for
    /// identical canonical residuals within one immutable engine.
    pub(super) fn prove_base_lower_bound(&self, polynomial: &Polynomial, bound: &BigInt) -> bool {
        if let Some((positive, negative, constant)) = polynomial.as_atom_difference()
            && let Some(best) = self.engine.matrix_bound(&positive, &negative)
            && best.add(&constant) >= *bound
        {
            return true;
        }
        if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
            let other = if sign == 1 {
                self.engine.matrix_bound(&atom, ZERO_ATOM)
            } else {
                self.engine.matrix_bound(ZERO_ATOM, &atom)
            };
            if let Some(best) = other
                && best.add(&constant) >= *bound
            {
                return true;
            }
        }
        // A premise can match the goal directly or after positive rational
        // scaling and a constant offset. Prepared strongest bounds subsume
        // weaker copies for this positive, threshold-monotone judgment too.
        if self.exact_bound_at_least(polynomial, bound) {
            return true;
        }
        let proportional = |stored: &Polynomial, stored_bound: &BigInt| {
            self.prove_proportional_lower_bound(polynomial, bound, stored, stored_bound)
        };
        let established = if let Some(prepared) = self.prepared_index() {
            let first = prepared.proportional.partition_point(|&position| {
                let (stored, _) =
                    &self.engine.bounds[prepared.exact_indices[position as usize] as usize];
                nonconstant_monomials_cmp(stored, polynomial).is_lt()
            });
            prepared.proportional[first..]
                .iter()
                .map(|&position| {
                    &self.engine.bounds[prepared.exact_indices[position as usize] as usize]
                })
                .take_while(|(stored, _)| nonconstant_monomials_cmp(stored, polynomial).is_eq())
                .any(|(stored, bound)| proportional(stored, bound))
        } else {
            self.engine
                .bounds
                .iter()
                .any(|(stored, bound)| proportional(stored, bound))
        };
        if established {
            return true;
        }
        if let Some(low) = self.polynomial_interval(polynomial).low
            && low >= *bound
        {
            return true;
        }
        if self
            .affine_difference_lower_bound(polynomial)
            .is_some_and(|low| low >= *bound)
        {
            return true;
        }
        // Horner adds no enclosure for canonical affine terms: each atom
        // occurs once, so its constant coefficient scales the same interval
        // already summed above. Matrix refinement can only tighten that earlier
        // interval, and Horner's depth cutoff can only weaken its enclosure.
        // Keep recursive/nonlinear Horner unchanged, including its cutoffs.
        if polynomial.terms.keys().all(|monomial| {
            monomial.is_empty() || (monomial.len() == 1 && monomial.values().next() == Some(&1))
        }) {
            return false;
        }
        self.horner_interval(polynomial, 0)
            .low
            .is_some_and(|low| low >= *bound)
    }

    /// Independent atom intervals lose sums of known differences. Consume each
    /// affine coefficient once, pairing positive and negative mass with proved
    /// matrix edges and bounding the remainder independently. Every selected
    /// edge contributes a nonnegative multiple of an established inequality;
    /// neither the requested goal nor a pending operation becomes a premise.
    ///
    /// This is a bounded sufficient enclosure, not a linear-programming solver.
    /// Prefer edges covering unbounded terms, then their improvement over unary
    /// bounds. An incomplete greedy decomposition stays unknown. It cannot
    /// weaken the independent enclosure already tried by the caller.
    fn affine_difference_lower_bound(&self, polynomial: &Polynomial) -> Option<BigInt> {
        let mut constant = BigInt::zero();
        let mut positive = Vec::new();
        let mut negative = Vec::new();
        for (monomial, coefficient) in &polynomial.terms {
            if monomial.is_empty() {
                constant = coefficient.clone();
                continue;
            }
            let mut factors = monomial.iter();
            let (atom, power) = factors.next()?;
            if *power != 1 || factors.next().is_some() {
                return None;
            }
            let interval = self.atom_interval(atom);
            if coefficient.is_negative() {
                negative.push((
                    atom,
                    coefficient.abs(),
                    interval.high.map(|high| high.negate()),
                ));
            } else {
                positive.push((atom, coefficient.clone(), interval.low));
            }
        }
        // The single-difference route already covers smaller affine terms.
        // Bound this optional search independently of source integer widths.
        if positive.len() + negative.len() < 3 || positive.len() + negative.len() > 128 {
            return None;
        }
        let mut edges = Vec::new();
        for (positive_position, (positive_atom, _, positive_low)) in positive.iter().enumerate() {
            for (negative_position, (negative_atom, _, negative_low)) in negative.iter().enumerate()
            {
                let Some(edge_bound) = self.engine.matrix_bound(positive_atom, negative_atom)
                else {
                    continue;
                };
                let unbounded =
                    usize::from(positive_low.is_none()) + usize::from(negative_low.is_none());
                let improvement = match (positive_low, negative_low) {
                    (Some(left), Some(right)) => edge_bound.sub(&left.add(right)),
                    _ => edge_bound.clone(),
                };
                edges.push((
                    unbounded,
                    improvement,
                    positive_position,
                    negative_position,
                    edge_bound,
                ));
            }
        }
        edges.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| right.1.cmp(&left.1)));
        for (_, _, positive_position, negative_position, edge_bound) in edges {
            let positive_mass = &mut positive[positive_position].1;
            let negative_mass = &mut negative[negative_position].1;
            let mass = positive_mass.clone().min(negative_mass.clone());
            constant = constant.add(&mass.mul(&edge_bound));
            *positive_mass = positive_mass.sub(&mass);
            *negative_mass = negative_mass.sub(&mass);
        }
        for (_, mass, lower_bound) in positive.into_iter().chain(negative) {
            if !mass.is_zero() {
                constant = constant.add(&mass.mul(&lower_bound?));
            }
        }
        Some(constant)
    }

    pub(in crate::proof_contracts::contract_entailment) fn polynomial_interval(
        &self,
        polynomial: &Polynomial,
    ) -> Interval {
        let mut total = Interval::constant(BigInt::zero());
        for (monomial, coefficient) in &polynomial.terms {
            let mut product = Interval::constant(BigInt::from_i64(1));
            for (atom, power) in monomial {
                let base = self.atom_interval(atom);
                product = product.multiply(&base.correlated_power(*power));
            }
            total = total.add(&product.scale(coefficient));
        }
        // Independent atom intervals forget correlations such as upper >
        // lower. Preserve a difference's established range when it becomes
        // an operand of another operation (notably a quotient).
        if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
            if let Some(low) = self.engine.matrix_bound(&positive, &negative) {
                let low = low.add(&constant);
                total.low = Some(total.low.map_or(low.clone(), |old| old.max(low)));
            }
            if let Some(high) = self.engine.matrix_bound(&negative, &positive) {
                let high = constant.sub(&high);
                total.high = Some(total.high.map_or(high.clone(), |old| old.min(high)));
            }
        }
        total
    }

    /// From S >= s and A*G - B*S = C, with A and B positive, infer
    /// G >= g only when B*s + C >= A*g. Cross-products check every
    /// nonconstant coefficient, and C retains both constant offsets. This
    /// consumes one existing premise; it does not recursively search bounds.
    pub(super) fn prove_proportional_lower_bound(
        &self,
        goal: &Polynomial,
        bound: &BigInt,
        stored: &Polynomial,
        stored_bound: &BigInt,
    ) -> bool {
        let Some((goal_coefficient, stored_coefficient)) =
            positive_coefficient_pairs(goal, stored).next()
        else {
            return false;
        };
        let goal_scale = stored_coefficient.abs();
        let stored_scale = goal_coefficient.abs();
        for (monomial, coefficient) in &goal.terms {
            if monomial.is_empty() {
                continue;
            }
            let Some(other) = stored.terms.get(monomial) else {
                return false;
            };
            if coefficient.mul(&goal_scale) != other.mul(&stored_scale) {
                return false;
            }
        }
        if stored
            .terms
            .keys()
            .any(|monomial| !monomial.is_empty() && !goal.terms.contains_key(monomial))
        {
            return false;
        }
        let constant = BTreeMap::new();
        let goal_constant = goal.terms.get(&constant).cloned().unwrap_or_default();
        let stored_constant = stored.terms.get(&constant).cloned().unwrap_or_default();
        let offset = goal_constant
            .mul(&goal_scale)
            .sub(&stored_constant.mul(&stored_scale));
        stored_bound.mul(&stored_scale).add(&offset) >= bound.mul(&goal_scale)
    }

    pub(super) fn prove_scaled_lower_bound(
        &self,
        goal: &Polynomial,
        bound: &BigInt,
        stored: &Polynomial,
        stored_bound: &BigInt,
    ) -> bool {
        let Some((goal_coefficient, stored_coefficient)) = positive_coefficient_pairs(goal, stored)
            .find(|(goal, stored)| goal.abs() != stored.abs())
        else {
            return false;
        };
        let goal_scale = stored_coefficient.abs();
        let stored_scale = goal_coefficient.abs();
        let Some(scaled_goal) = goal.checked_mul(&Polynomial::constant(goal_scale.clone())) else {
            return false;
        };
        let Some(scaled_stored) = stored.checked_mul(&Polynomial::constant(stored_scale.clone()))
        else {
            return false;
        };
        // S >= c and A*G - B*S >= A*b - B*c, for positive A/B, imply G >= b.
        // When G is integral, A*G > A*(b-1) suffices: the weakest integer
        // threshold is A*b-(A-1). This is not available to legacy adapters
        // whose parameter atoms may denote nonintegers.
        // Keep this nonrecursive: never search combinations of stored premises.
        let integer_slack = if self.engine.strict_symbol_bindings.is_some()
            && self.engine.strict_symbol_bindings_valid
            && !self.engine.proof_integer_formation
        {
            goal_scale.sub(&BigInt::from_i64(1))
        } else {
            BigInt::zero()
        };
        self.prove_base_lower_bound(
            &scaled_goal.sub(&scaled_stored),
            &bound
                .mul(&goal_scale)
                .sub(&stored_bound.mul(&stored_scale))
                .sub(&integer_slack),
        )
    }

    pub(super) fn horner_interval(&self, polynomial: &Polynomial, depth: usize) -> Interval {
        if let Some(constant) = polynomial.constant_value() {
            return Interval::constant(constant);
        }
        // Private search provision: refusal establishes no bound. Partitioning
        // removes one atom per level and never expands a product into new terms.
        if depth >= 128 {
            return Interval::unbounded();
        }
        let Some(atom) = polynomial
            .terms
            .keys()
            .find_map(|monomial| monomial.keys().next())
        else {
            return Interval::unbounded();
        };
        let mut coefficients = BTreeMap::<u32, Polynomial>::new();
        for (monomial, coefficient) in &polynomial.terms {
            let mut remainder = monomial.clone();
            let power = remainder.remove(atom).unwrap_or(0);
            coefficients
                .entry(power)
                .or_default()
                .terms
                .insert(remainder, coefficient.clone());
        }
        let atom_interval = self.atom_interval(atom);
        let mut powers = coefficients.iter().rev();
        let Some((&highest_power, leading)) = powers.next() else {
            return Interval::unbounded();
        };
        if highest_power > 128 {
            return Interval::unbounded();
        }
        let mut previous_power = highest_power;
        let mut interval = self.horner_interval(leading, depth + 1);
        for (&power, coefficient) in powers {
            interval = interval
                .multiply(&atom_interval.correlated_power(previous_power - power))
                .add(&self.horner_interval(coefficient, depth + 1));
            previous_power = power;
        }
        interval.multiply(&atom_interval.correlated_power(previous_power))
    }
}

fn nonconstant_monomials_cmp(left: &Polynomial, right: &Polynomial) -> std::cmp::Ordering {
    left.terms
        .keys()
        .filter(|monomial| !monomial.is_empty())
        .cmp(right.terms.keys().filter(|monomial| !monomial.is_empty()))
}

/// Positive rational alignment is shared by the exact proportional-premise
/// query and the existing scaled residual query. Zero/opposite-sign pairs
/// cannot justify multiplying an inequality in its original direction.
fn positive_coefficient_pairs<'a>(
    goal: &'a Polynomial,
    stored: &'a Polynomial,
) -> impl Iterator<Item = (&'a BigInt, &'a BigInt)> {
    goal.terms
        .iter()
        .filter_map(move |(monomial, coefficient)| {
            if monomial.is_empty() {
                return None;
            }
            let other = stored.terms.get(monomial)?;
            (!coefficient.is_zero()
                && !other.is_zero()
                && coefficient.is_negative() == other.is_negative())
            .then_some((coefficient, other))
        })
}
