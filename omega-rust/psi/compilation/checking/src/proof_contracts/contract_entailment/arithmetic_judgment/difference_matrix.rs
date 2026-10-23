//! Difference-matrix closure for the arithmetic engine: atom-pair bound
//! seeding and transitive closure, nonzero-difference proofs, interval
//! reads of polynomials, and substitution across the polynomial surface.
use super::super::BigInt;
use super::{Engine, Polynomial, SUBSTITUTION_ROUNDS, ZERO_ATOM};
use crate::proof_contracts::arithmetic_domains::BigIntInterval as Interval;

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
        // A premise can match the goal directly or after positive rational
        // scaling and a constant offset. Preserve correlated sums rather than
        // replacing them with independent intervals. This remains a single
        // established premise, including when an outer query asks a residual.
        for (stored, stored_bound) in &self.bounds {
            if (stored == polynomial && stored_bound >= bound)
                || self.prove_proportional_lower_bound(polynomial, bound, stored, stored_bound)
            {
                return true;
            }
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
                let Some(edge_bound) = self.matrix_bound(positive_atom, negative_atom) else {
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

    /// The closed lower bound of `from - to`. Atoms in different components
    /// relate only through the zero atom, so their bound is the two-hop sum
    /// `(from - 0) + (0 - to)`; the component closures below already hold
    /// every other path.
    pub(super) fn matrix_bound(&self, from: &str, to: &str) -> Option<BigInt> {
        if from == to {
            return Some(BigInt::zero());
        }
        if let Some(bound) = self.recorded_bound(from, to) {
            return Some(bound);
        }
        if from == ZERO_ATOM || to == ZERO_ATOM {
            return None;
        }
        let through_zero = self.recorded_bound(from, ZERO_ATOM)?;
        Some(through_zero.add(&self.recorded_bound(ZERO_ATOM, to)?))
    }

    fn recorded_bound(&self, from: &str, to: &str) -> Option<BigInt> {
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

    /// Transitive closure of the recorded differences, one connected
    /// component at a time. Atoms are joined by their recorded edges; the
    /// zero atom joins every component and is excluded from connectivity. A
    /// path between two atoms that leaves their component passes through
    /// zero twice, and the excursion is a cycle at zero whose weight cannot
    /// be positive in a satisfiable system, so it never improves the path;
    /// a positive cycle through zero spanning two components decomposes into
    /// a positive cycle within one of them. The closure of each component
    /// with zero therefore restricts the whole closure exactly, and
    /// `matrix_bound` supplies the cross-component two-hop through zero.
    /// The roster can hold thousands of atoms related only to zero (every
    /// integer leaf of a large record), where the whole-roster cubic
    /// closure did not finish; component closures stay cubic in each
    /// component's size.
    pub(super) fn close_matrix(&mut self) {
        let atoms: Vec<String> = self
            .matrix
            .keys()
            .filter(|atom| atom.as_str() != ZERO_ATOM)
            .cloned()
            .collect();
        let position: std::collections::BTreeMap<&str, usize> = atoms
            .iter()
            .enumerate()
            .map(|(index, atom)| (atom.as_str(), index))
            .collect();
        let mut parent: Vec<usize> = (0..atoms.len()).collect();
        fn root(parent: &mut [usize], mut index: usize) -> usize {
            while parent[index] != index {
                parent[index] = parent[parent[index]];
                index = parent[index];
            }
            index
        }
        for (from, row) in &self.matrix {
            let Some(&from_index) = position.get(from.as_str()) else {
                continue;
            };
            for to in row.keys() {
                let Some(&to_index) = position.get(to.as_str()) else {
                    continue;
                };
                let (left, right) = (root(&mut parent, from_index), root(&mut parent, to_index));
                if left != right {
                    parent[left] = right;
                }
            }
        }
        let mut components: std::collections::BTreeMap<usize, Vec<String>> =
            std::collections::BTreeMap::new();
        for (index, atom) in atoms.iter().enumerate() {
            components
                .entry(root(&mut parent, index))
                .or_default()
                .push(atom.clone());
        }
        let has_zero = self.matrix.contains_key(ZERO_ATOM);
        for mut members in components.into_values() {
            if has_zero {
                members.push(ZERO_ATOM.to_owned());
            }
            self.close_members(&members);
        }
    }

    fn close_members(&mut self, members: &[String]) {
        for via in members {
            for from in members {
                let Some(first) = self.recorded_bound(from, via) else {
                    continue;
                };
                for to in members {
                    let Some(second) = self.recorded_bound(via, to) else {
                        continue;
                    };
                    let combined = first.add(&second);
                    if from == to {
                        if !combined.is_negative() && !combined.is_zero() {
                            self.requires_unsatisfiable = true;
                        }
                        continue;
                    }
                    let current = self.recorded_bound(from, to);
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
