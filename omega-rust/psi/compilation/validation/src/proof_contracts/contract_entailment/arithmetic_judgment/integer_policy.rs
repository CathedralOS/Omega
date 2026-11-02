//! Completed finite-policy values retain reduction and clamping semantics.
//!
//! A mathematical result may replace its representative only when the live
//! hypotheses prove it lands. Otherwise opaque policy atoms retain their actual
//! carrier intervals, including intervals wholly inside one wrapping band.

use super::{BigInt, Engine, OpaqueTerm, Polynomial};
use crate::proof_contracts::arithmetic_domains::BigIntInterval as Interval;
use numerics::arithmetic::ArithmeticDomain;
use typed_trees::types::PrimitiveType;

impl Engine<'_> {
    /// Complete a selected finite integer calculation once. A representative
    /// is an actual value, not the unbounded polynomial that produced it.
    /// Same-carrier wrapping children can expand only inside the residue
    /// polynomial: quotient/shift atoms and foreign policies remain values.
    pub(in crate::proof_contracts::contract_entailment) fn integer_policy_value(
        &mut self,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        value: Polynomial,
    ) -> Option<Polynomial> {
        if !primitive.is_fixed_integer()
            || !matches!(
                domain,
                ArithmeticDomain::Wrapping | ArithmeticDomain::Saturating
            )
        {
            return None;
        }
        let value = self.substituted(&value);
        if self.integer_value_lands(primitive, &value) {
            return Some(value);
        }
        let (minimum, maximum) = super::super::ranking_range::integer_carrier_bounds(primitive)?;
        let value = if domain == ArithmeticDomain::Wrapping {
            self.wrapping_residue(
                primitive,
                value,
                &maximum.sub(&minimum).add(&BigInt::from_i64(1)),
            )?
        } else {
            value
        };
        if let Some(value) = value.constant_value() {
            return Some(Polynomial::constant(complete_integer_policy(
                domain, &value, &minimum, &maximum,
            )?));
        }
        let atom = format!("\0integer-policy:{primitive:?}:{domain:?}:{value:?}");
        let interval = self.integer_policy_interval(primitive, domain, &value);
        self.register_opaque_term(
            atom.clone(),
            OpaqueTerm::IntegerPolicy {
                primitive,
                domain,
                value,
            },
        );
        self.arithmetic_intervals.insert(atom.clone(), interval);
        Some(Polynomial::atom(atom))
    }

    fn wrapping_residue(
        &self,
        primitive: PrimitiveType,
        value: Polynomial,
        modulus: &BigInt,
    ) -> Option<Polynomial> {
        let mut substitutions = super::BTreeMap::new();
        for monomial in value.terms.keys() {
            for atom in monomial.keys() {
                let replacement = self
                    .opaque_terms
                    .iter()
                    .find_map(|(known, term)| {
                        if known != atom {
                            return None;
                        }
                        match term {
                            OpaqueTerm::IntegerPolicy {
                                primitive: source,
                                domain: ArithmeticDomain::Wrapping,
                                value,
                            } if *source == primitive => Some(value.clone()),
                            _ => None,
                        }
                    })
                    .unwrap_or_else(|| Polynomial::atom(atom.clone()));
                substitutions.insert(atom.clone(), replacement);
            }
        }
        let mut value =
            super::super::inductive_judgment::apply_argument_map(&value, &substitutions)?;
        for coefficient in value.terms.values_mut() {
            let (_, remainder) = coefficient.div_rem(modulus)?;
            *coefficient = if remainder.is_negative() {
                remainder.add(modulus)
            } else {
                remainder
            };
        }
        value.terms.retain(|_, coefficient| !coefficient.is_zero());
        Some(value)
    }

    pub(super) fn integer_policy_interval(
        &self,
        primitive: PrimitiveType,
        domain: ArithmeticDomain,
        value: &Polynomial,
    ) -> Interval {
        let Some((minimum, maximum)) =
            super::super::ranking_range::integer_carrier_bounds(primitive)
        else {
            return Interval::unbounded();
        };
        let value = self.substituted(value);
        if let Some(value) = value.constant_value()
            && let Some(value) = complete_integer_policy(domain, &value, &minimum, &maximum)
        {
            return Interval::constant(value);
        }
        if domain == ArithmeticDomain::Saturating {
            let interval = self.polynomial_interval(&value);
            return Interval {
                low: Some(interval.low.map_or_else(
                    || minimum.clone(),
                    |value| value.max(minimum.clone()).min(maximum.clone()),
                )),
                high: Some(interval.high.map_or_else(
                    || maximum.clone(),
                    |value| value.max(minimum).min(maximum.clone()),
                )),
            };
        }
        // Endpoint reduction is valid only when the entire interval stays
        // inside one modular band. Equal widths rule out every wrap boundary,
        // including an interval spanning multiple complete carrier ranges.
        let interval = self.polynomial_interval(&value);
        if let (Some(low), Some(high)) = (interval.low, interval.high)
            && let (Some(wrapped_low), Some(wrapped_high)) = (
                complete_integer_policy(domain, &low, &minimum, &maximum),
                complete_integer_policy(domain, &high, &minimum, &maximum),
            )
            && high.sub(&low) == wrapped_high.sub(&wrapped_low)
        {
            return Interval {
                low: Some(wrapped_low),
                high: Some(wrapped_high),
            };
        }
        Interval {
            low: Some(minimum),
            high: Some(maximum),
        }
    }
}

fn complete_integer_policy(
    domain: ArithmeticDomain,
    value: &BigInt,
    minimum: &BigInt,
    maximum: &BigInt,
) -> Option<BigInt> {
    match domain {
        ArithmeticDomain::Wrapping => {
            let modulus = maximum.sub(minimum).add(&BigInt::from_i64(1));
            let (_, remainder) = value.sub(minimum).div_rem(&modulus)?;
            let residue = if remainder.is_negative() {
                remainder.add(&modulus)
            } else {
                remainder
            };
            Some(minimum.add(&residue))
        }
        ArithmeticDomain::Saturating => {
            Some(value.clone().max(minimum.clone()).min(maximum.clone()))
        }
        _ => None,
    }
}
