//! Difference-matrix closure for the arithmetic engine: atom-pair bound
//! seeding and transitive closure, nonzero-difference proofs, interval
//! reads of polynomials, and substitution across the polynomial surface.
use super::super::BigInt;
use super::{Engine, Polynomial, SUBSTITUTION_ROUNDS, ZERO_ATOM};
use crate::proof_contracts::arithmetic_domains::BigIntInterval as Interval;

impl<'program> Engine<'program> {
    pub(super) fn is_known_nonzero(&self, difference: &Polynomial) -> bool {
        let opposite = difference.neg();
        self.nonzero_differences.iter().any(|known| {
            // Hypothesis installation is additive: later equations can rename
            // atoms, but cannot change the captured difference's meaning.
            let known = self.substituted(known);
            known == *difference || known == opposite
        })
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
        self.bounds.iter().any(|(stored, stored_bound)| {
            // P >= b and goal - P >= requested - b imply goal >= requested.
            // Do not recursively search combinations of stored hypotheses.
            self.prove_base_lower_bound(&polynomial.sub(stored), &bound.sub(stored_bound))
                || self.prove_scaled_lower_bound(polynomial, bound, stored, stored_bound)
        })
    }

    /// Difference bounds, exact hypotheses, and independent intervals only.
    ///
    /// This query is pure: contradiction checking reuses failed queries for
    /// identical canonical residuals within one immutable engine.
    pub(super) fn prove_base_lower_bound(&self, polynomial: &Polynomial, bound: &BigInt) -> bool {
        if let Some((positive, negative, constant)) = polynomial.as_atom_difference()
            && let Some(best) = self.matrix_bound(&positive, &negative)
            && best.add(&constant) >= *bound
        {
            return true;
        }
        if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
            let other = if sign == 1 {
                self.matrix_bound(&atom, ZERO_ATOM)
            } else {
                self.matrix_bound(ZERO_ATOM, &atom)
            };
            if let Some(best) = other
                && best.add(&constant) >= *bound
            {
                return true;
            }
        }
        // A stored hypothesis bound whose polynomial IS the goal polynomial
        // subsumes it directly. This is the shape induction hypotheses
        // arrive in: general polynomial equations (e.g. `2*result - P >= 0`)
        // that fit neither the difference-bound matrix nor the interval
        // evaluator, but whose canonical form matches the goal exactly.
        for (stored, stored_bound) in &self.bounds {
            if stored == polynomial && stored_bound >= bound {
                return true;
            }
        }
        if let Some(low) = self.polynomial_interval(polynomial).low
            && low >= *bound
        {
            return true;
        }
        self.horner_interval(polynomial, 0)
            .low
            .is_some_and(|low| low >= *bound)
    }

    /// P >= b contradicts -P >= 1-b over integers. Apply the same standalone,
    /// residual and scaled judgments as `prove_at_least`, without asking the
    /// identical residual question twice. No new combinations are searched.
    pub(super) fn bounds_are_inconsistent(&self) -> bool {
        for (bound_position, (polynomial, bound)) in self.bounds.iter().enumerate() {
            let complement = polynomial.neg();
            let complement_bound = BigInt::from_i64(1).sub(bound);
            if self.prove_base_lower_bound(&complement, &complement_bound) {
                return true;
            }
            for (stored_position, (stored, stored_bound)) in self.bounds.iter().enumerate() {
                // The reversed pair asks exactly -P-Q >= 1-b-c too. Earlier
                // rows completed false or we already returned; canonical exact
                // arithmetic and immutable engine state make that result reusable.
                // Keep the diagonal and every directed scaled query: scaling's
                // coefficient selection and integer slack need not be symmetric.
                if (stored_position >= bound_position
                    && self.prove_base_lower_bound(
                        &complement.sub(stored),
                        &complement_bound.sub(stored_bound),
                    ))
                    || self.prove_scaled_lower_bound(
                        &complement,
                        &complement_bound,
                        stored,
                        stored_bound,
                    )
                {
                    return true;
                }
            }
        }
        false
    }

    pub(super) fn polynomial_interval(&self, polynomial: &Polynomial) -> Interval {
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
            if let Some(low) = self.matrix_bound(&positive, &negative) {
                let low = low.add(&constant);
                total.low = Some(total.low.map_or(low.clone(), |old| old.max(low)));
            }
            if let Some(high) = self.matrix_bound(&negative, &positive) {
                let high = constant.sub(&high);
                total.high = Some(total.high.map_or(high.clone(), |old| old.min(high)));
            }
        }
        total
    }

    pub(super) fn atom_interval(&self, atom: &str) -> Interval {
        if let Some(interval) = self.arithmetic_intervals.get(atom) {
            return interval.clone();
        }
        let mut low = self.matrix_bound(atom, ZERO_ATOM);
        if self
            .unsigned_atoms
            .iter()
            .any(|candidate| candidate == atom)
        {
            // An unsigned atom is nonnegative before the hypothesis matrix
            // seeds that bound, so a term minted ahead of installation still
            // sees the operand's true sign.
            low = Some(low.map_or_else(BigInt::zero, |bound| bound.max(BigInt::zero())));
        }
        Interval {
            low,
            high: self
                .matrix_bound(ZERO_ATOM, atom)
                .map(|bound| bound.negate()),
        }
    }

    pub(super) fn matrix_bound(&self, from: &str, to: &str) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        self.matrix.get(from).and_then(|row| row.get(to)).cloned()
    }

    pub(super) fn seed_matrix(&mut self) {
        for atom in self.unsigned_atoms.clone() {
            self.record_difference(&atom, ZERO_ATOM, BigInt::zero());
        }
        let arithmetic_atoms: Vec<(String, Interval)> = self
            .arithmetic_intervals
            .iter()
            .map(|(atom, interval)| (atom.clone(), interval.clone()))
            .collect();
        for (atom, interval) in arithmetic_atoms {
            if let Some(low) = interval.low {
                self.record_difference(&atom, ZERO_ATOM, low);
            }
            if let Some(high) = interval.high {
                self.record_difference(ZERO_ATOM, &atom, high.negate());
            }
        }
        for (polynomial, bound) in self.bounds.clone() {
            if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
                self.record_difference(&positive, &negative, bound.sub(&constant));
            }
            if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
                let edge = bound.sub(&constant);
                if sign == 1 {
                    self.record_difference(&atom, ZERO_ATOM, edge);
                } else {
                    self.record_difference(ZERO_ATOM, &atom, edge);
                }
            }
        }
    }

    pub(super) fn record_difference(&mut self, from: &str, to: &str, bound: BigInt) {
        let row = self.matrix.entry(from.to_owned()).or_default();
        match row.entry(to.to_owned()) {
            std::collections::btree_map::Entry::Vacant(slot) => {
                slot.insert(bound);
            }
            std::collections::btree_map::Entry::Occupied(mut slot) => {
                if bound > *slot.get() {
                    slot.insert(bound);
                }
            }
        }
        self.matrix.entry(to.to_owned()).or_default();
    }

    pub(super) fn close_matrix(&mut self) {
        let atoms: Vec<String> = self.matrix.keys().cloned().collect();
        for via in &atoms {
            for from in &atoms {
                let Some(first) = self.matrix_bound(from, via) else {
                    continue;
                };
                for to in &atoms {
                    let Some(second) = self.matrix_bound(via, to) else {
                        continue;
                    };
                    let combined = first.add(&second);
                    if from == to {
                        if !combined.is_negative() && !combined.is_zero() {
                            self.requires_unsatisfiable = true;
                        }
                        continue;
                    }
                    let current = self.matrix_bound(from, to);
                    if current.is_none() || combined > current.unwrap() {
                        self.record_difference(from, to, combined);
                    }
                }
            }
        }
    }

    pub(super) fn harvest_substitution(&mut self, left: &Polynomial, right: &Polynomial) {
        for (candidate, replacement) in [(left, right), (right, left)] {
            if let Some((atom, 1, constant)) = candidate.as_single_atom()
                && constant.is_zero()
            {
                let occurs = replacement
                    .terms
                    .keys()
                    .any(|monomial| monomial.contains_key(&atom));
                if !occurs && !self.substitutions.contains_key(&atom) {
                    self.substitutions.insert(atom, replacement.clone());
                    return;
                }
            }
        }
    }

    pub(in crate::proof_contracts::contract_entailment) fn substituted(
        &self,
        polynomial: &Polynomial,
    ) -> Polynomial {
        let mut current = polynomial.clone();
        for _ in 0..SUBSTITUTION_ROUNDS {
            let mut changed = false;
            let mut next = Polynomial::default();
            let mut overflowed = false;
            for (monomial, coefficient) in &current.terms {
                let mut piece = Polynomial::constant(coefficient.clone());
                for (atom, power) in monomial {
                    let base = match self.substitutions.get(atom) {
                        Some(replacement) => {
                            changed = true;
                            replacement.clone()
                        }
                        None => Polynomial::atom(atom.clone()),
                    };
                    for _ in 0..*power {
                        match piece.checked_mul(&base) {
                            Some(product) => piece = product,
                            None => {
                                overflowed = true;
                                break;
                            }
                        }
                    }
                    if overflowed {
                        break;
                    }
                }
                if overflowed {
                    break;
                }
                next = next.add(&piece);
            }
            if overflowed {
                return current;
            }
            current = next;
            if !changed {
                break;
            }
        }
        current
    }
}
