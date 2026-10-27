//! Difference-matrix closure for the arithmetic engine: atom-pair bound
//! seeding and transitive closure, nonzero-difference proofs, interval
//! reads of polynomials, and substitution across the polynomial surface.
use super::super::BigInt;
use super::{Engine, Polynomial, SUBSTITUTION_ROUNDS, ZERO_ATOM};
use crate::proof_contracts::arithmetic_domains::BigIntInterval as Interval;

mod bounds;
pub(super) use bounds::DifferenceBounds;

impl<'program> Engine<'program> {
    /// Integer disequality follows from a retained unequal difference or a
    /// strict bound in either direction. Unknown is not nonzero evidence.
    pub(in crate::proof_contracts::contract_entailment) fn proves_nonzero(
        &self,
        polynomial: &Polynomial,
    ) -> bool {
        let one = BigInt::from_i64(1);
        self.is_known_nonzero(polynomial)
            || self.prove_at_least(polynomial, &one)
            || self.prove_at_least(&polynomial.neg(), &one)
    }

    pub(in crate::proof_contracts::contract_entailment) fn is_known_nonzero(
        &self,
        difference: &Polynomial,
    ) -> bool {
        let opposite = difference.neg();
        self.nonzero_differences.iter().any(|known| {
            // Hypothesis installation is additive: later equations can rename
            // atoms, but cannot change the captured difference's meaning.
            let known = self.substituted(known);
            known == *difference || known == opposite
        })
    }

    pub(in crate::proof_contracts::contract_entailment) fn prove_at_least(
        &self,
        polynomial: &Polynomial,
        bound: &BigInt,
    ) -> bool {
        super::LowerBoundQueries::direct(self).prove_at_least(polynomial, bound)
    }

    pub(super) fn prove_base_lower_bound(&self, polynomial: &Polynomial, bound: &BigInt) -> bool {
        super::LowerBoundQueries::direct(self).prove_base_lower_bound(polynomial, bound)
    }

    pub(super) fn prove_scaled_lower_bound(
        &self,
        goal: &Polynomial,
        bound: &BigInt,
        stored: &Polynomial,
        stored_bound: &BigInt,
    ) -> bool {
        super::LowerBoundQueries::direct(self).prove_scaled_lower_bound(
            goal,
            bound,
            stored,
            stored_bound,
        )
    }

    /// P >= b contradicts -P >= 1-b over integers. Apply the same standalone,
    /// residual and scaled judgments as `prove_at_least`, without asking the
    /// identical residual question twice. No new combinations are searched.
    pub(super) fn bounds_are_inconsistent(&self) -> bool {
        let queries = self.lower_bound_queries();
        let exact = queries.prepared_exact_bounds();
        for (bound_position, (polynomial, bound)) in exact.iter().enumerate() {
            let complement = polynomial.neg();
            let complement_bound = BigInt::from_i64(1).sub(bound);
            if queries.prove_base_lower_bound(&complement, &complement_bound) {
                return true;
            }
            for (stored_position, (stored, stored_bound)) in exact.iter().enumerate() {
                // The reversed pair asks exactly -P-Q >= 1-b-c too. Earlier
                // rows completed false or we already returned; canonical exact
                // arithmetic and immutable engine state make that result reusable.
                // Keep the diagonal and every directed scaled query: scaling's
                // coefficient selection and integer slack need not be symmetric.
                if (stored_position >= bound_position
                    && queries.prove_base_lower_bound(
                        &complement.sub(stored),
                        &complement_bound.sub(stored_bound),
                    ))
                    || queries.prove_scaled_lower_bound(
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

    pub(in crate::proof_contracts::contract_entailment) fn polynomial_interval(
        &self,
        polynomial: &Polynomial,
    ) -> Interval {
        super::LowerBoundQueries::direct(self).polynomial_interval(polynomial)
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

    /// Components communicate through zero without materializing every pair.
    /// Use the same query for indexed incremental updates and named callers.
    pub(super) fn matrix_bound(&self, from: &str, to: &str) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        self.matrix_bound_indices(self.matrix.position(from)?, self.matrix.position(to)?)
    }

    fn matrix_bound_indices(&self, from: usize, to: usize) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        if let Some(bound) = self.matrix.stored_bound_indices(from, to) {
            return Some(bound.clone());
        }
        let zero = self.matrix.position(ZERO_ATOM)?;
        if from == zero || to == zero {
            return None;
        }
        let through_zero = self.matrix.stored_bound_indices(from, zero)?;
        Some(through_zero.add(self.matrix.stored_bound_indices(zero, to)?))
    }

    /// Matrix writes are additive. A consistent retained matrix is already
    /// transitively closed, so one stronger seed needs only paths containing
    /// that edge once. An inconsistent graph retains the original closure and
    /// its order-dependent values; speculative updates never enter that route.
    pub(super) fn refresh_difference_bounds(&mut self) {
        if self.requires_unsatisfiable {
            self.seed_matrix();
            self.close_matrix();
            return;
        }
        let mut original = None;
        for (from, to, bound) in self.difference_seeds() {
            if self
                .matrix
                .stored_bound(&from, &to)
                .is_some_and(|previous| previous >= &bound)
            {
                continue;
            }
            // Clone only once, before this refresh's first changed seed. A
            // later edge can close a positive cycle through earlier new edges.
            // Restoring only that edge's input would change old-route history.
            let original_matrix = original.get_or_insert_with(|| self.matrix.clone());
            if !self.strengthen_closed_difference(&from, &to, bound) {
                self.matrix = std::mem::take(original_matrix);
                self.seed_matrix();
                self.close_matrix();
                return;
            }
        }
    }

    fn difference_seeds(&self) -> Vec<(String, String, BigInt)> {
        let mut seeds = Vec::new();
        for atom in &self.unsigned_atoms {
            seeds.push((atom.clone(), ZERO_ATOM.to_owned(), BigInt::zero()));
        }
        for (atom, interval) in &self.arithmetic_intervals {
            if let Some(low) = &interval.low {
                seeds.push((atom.clone(), ZERO_ATOM.to_owned(), low.clone()));
            }
            if let Some(high) = &interval.high {
                seeds.push((ZERO_ATOM.to_owned(), atom.clone(), high.negate()));
            }
        }
        for (polynomial, bound) in &self.bounds {
            if let Some((positive, negative, constant)) = polynomial.as_atom_difference() {
                seeds.push((positive, negative, bound.sub(&constant)));
            }
            if let Some((atom, sign, constant)) = polynomial.as_single_atom() {
                let edge = bound.sub(&constant);
                if sign == 1 {
                    seeds.push((atom, ZERO_ATOM.to_owned(), edge));
                } else {
                    seeds.push((ZERO_ATOM.to_owned(), atom, edge));
                }
            }
        }
        seeds
    }

    fn seed_matrix(&mut self) -> bool {
        let mut changed = false;
        for (from, to, bound) in self.difference_seeds() {
            changed |= self.record_difference(&from, &to, bound);
        }
        changed
    }

    /// Extend a closed consistent max-plus graph by u - v >= bound. A new
    /// edge can occur at most once on an improving path when the old reverse
    /// path plus this edge is nonpositive. Capture both old frontiers before
    /// writing, including virtual zero diagonals for previously unseen vertices.
    fn strengthen_closed_difference(&mut self, from: &str, to: &str, bound: BigInt) -> bool {
        if self
            .matrix_bound(to, from)
            .is_some_and(|reverse| reverse.add(&bound) > BigInt::zero())
        {
            // Do not set unsatisfiable here: the original seed/close route owns
            // historical diagonal handling and inconsistent graph behavior.
            return false;
        }
        if from == to {
            // Nonpositive diagonal seeds cannot improve paths, but still retain
            // the original vertex and stored diagonal inventory.
            self.record_difference(from, to, bound);
            return true;
        }
        // Capture the old lexical vertex inventory before introducing either
        // endpoint. Stable indices never determine semantic iteration order.
        let atoms = self.matrix.lexical_indices().to_vec();
        let from = self.matrix.intern(from);
        let to = self.matrix.intern(to);
        let mut incoming = vec![(from, BigInt::zero())];
        let mut outgoing = vec![(to, BigInt::zero())];
        for atom in atoms {
            if atom != from
                && let Some(value) = self.matrix_bound_indices(atom, from)
            {
                incoming.push((atom, value));
            }
            if atom != to
                && let Some(value) = self.matrix_bound_indices(to, atom)
            {
                outgoing.push((atom, value));
            }
        }
        self.matrix.record_indices(from, to, bound.clone());
        for (source, prefix) in incoming {
            let through_edge = prefix.add(&bound);
            for (destination, suffix) in &outgoing {
                if source != *destination {
                    self.matrix
                        .record_indices(source, *destination, through_edge.add(suffix));
                }
            }
        }
        true
    }

    fn record_difference(&mut self, from: &str, to: &str, bound: BigInt) -> bool {
        self.matrix.record(from, to, bound)
    }

    /// Close connected components with zero, preserving lexical iteration.
    /// Every path between components passes through zero. An excursion from
    /// zero back to zero cannot improve a satisfiable bound; a positive cycle
    /// already contradicts one component. Named and indexed queries therefore
    /// recover cross-component bounds through their two stored zero edges.
    pub(super) fn close_matrix(&mut self) {
        let zero = self.matrix.position(ZERO_ATOM);
        let atoms: Vec<usize> = self
            .matrix
            .lexical_indices()
            .iter()
            .copied()
            .filter(|index| Some(*index) != zero)
            .collect();
        let mut lexical_position = vec![0; self.matrix.lexical_indices().len()];
        for (position, atom) in atoms.iter().enumerate() {
            lexical_position[*atom] = position;
        }
        let mut parents: Vec<usize> = (0..atoms.len()).collect();
        fn component_root(parents: &mut [usize], mut index: usize) -> usize {
            while parents[index] != index {
                parents[index] = parents[parents[index]];
                index = parents[index];
            }
            index
        }
        for from in &atoms {
            let mut destinations: Vec<usize> = self
                .matrix
                .stored_targets(*from)
                .filter(|index| Some(*index) != zero)
                .collect();
            destinations.sort_unstable_by_key(|index| lexical_position[*index]);
            for to in destinations {
                let left = component_root(&mut parents, lexical_position[*from]);
                let right = component_root(&mut parents, lexical_position[to]);
                if left != right {
                    parents[left] = right;
                }
            }
        }
        let mut components = vec![Vec::new(); atoms.len()];
        for (position, atom) in atoms.into_iter().enumerate() {
            components[component_root(&mut parents, position)].push(atom);
        }
        for mut members in components.into_iter().filter(|members| !members.is_empty()) {
            if let Some(zero) = zero {
                members.push(zero);
            }
            self.close_members(&members);
        }
    }

    fn close_members(&mut self, members: &[usize]) {
        for via in members {
            for from in members {
                let Some(first) = self.matrix.stored_bound_indices(*from, *via).cloned() else {
                    continue;
                };
                for to in members {
                    let Some(second) = self.matrix.stored_bound_indices(*via, *to).cloned() else {
                        continue;
                    };
                    let combined = first.add(&second);
                    if from == to {
                        if !combined.is_negative() && !combined.is_zero() {
                            self.requires_unsatisfiable = true;
                        }
                        continue;
                    }
                    self.matrix.record_indices(*from, *to, combined);
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
