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
    pub(super) fn prove_scaled_lower_bound(
        &self,
        goal: &Polynomial,
        bound: &BigInt,
        stored: &Polynomial,
        stored_bound: &BigInt,
    ) -> bool {
        let Some((goal_coefficient, stored_coefficient)) =
            goal.terms.iter().find_map(|(monomial, coefficient)| {
                if monomial.is_empty() {
                    return None;
                }
                let other = stored.terms.get(monomial)?;
                (!coefficient.is_zero()
                    && !other.is_zero()
                    && coefficient.abs() != other.abs()
                    && coefficient.is_negative() == other.is_negative())
                .then_some((coefficient, other))
            })
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
