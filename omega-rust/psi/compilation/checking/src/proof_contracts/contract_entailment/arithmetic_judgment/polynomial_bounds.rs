//! Sufficient polynomial bounds from established atom intervals and one premise.
//!
//! Scaling aligns one nonconstant coefficient without division. Strict integer
//! scopes may round a scaled threshold using the discreteness of the goal;
//! legacy scopes can contain nonintegers and retain the unrounded threshold.
//! Horner enclosure retains useful repeated-factor relationships such as
//! `n*n - n = n*(n-1)`. Neither route installs a queried goal as a premise;
//! both supplement the existing monomial enclosure and may decline a true bound.
use super::{BTreeMap, BigInt, Engine, Interval, Polynomial};

impl Engine<'_> {
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
        let integer_slack = if self.strict_symbol_bindings.is_some()
            && self.strict_symbol_bindings_valid
            && !self.proof_integer_formation
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
