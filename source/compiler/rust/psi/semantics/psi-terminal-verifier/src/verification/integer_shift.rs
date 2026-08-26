//! Exact-shift reduction orchestration.
//!
//! The direct-chain and cross-family reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. This parent owns precedence, not their algorithms.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, PropositionContext, ScalarTerm, ValueId};

use super::{canonical_conjunction, integer_value_cmp, known_integer_term_value};

mod chains;
mod composition;

use chains::{
    append_exact_shift_left_value_bounds, exact_known_shift_count, known_shift_count_maximum,
    landed_exact_shift_count,
};
pub(super) use chains::{
    exact_integer_cast_then_shift_left_chain_obligation,
    exact_integer_mixed_shift_chain_cast_obligation, exact_integer_mixed_shift_chain_obligation,
    exact_integer_shift_left_chain_cast_obligation, exact_integer_shift_left_chain_obligation,
    exact_integer_shift_prefix_interval_obligation,
    exact_integer_shift_right_chain_cast_obligation, exact_integer_shifted_interval_obligation,
};
#[cfg(test)]
pub(super) use chains::{
    exact_integer_cumulative_shift_left_obligation, exact_integer_mixed_shift_preimage,
    exact_integer_shift_right_chain_cast_interval_obligation,
};
pub(super) use composition::{
    exact_integer_affine_cast_shift_obligation,
    exact_integer_arithmetic_then_shift_chain_obligation,
    exact_integer_cast_chain_then_shift_suffix_obligation,
    exact_integer_cast_then_mixed_shift_chain_obligation,
    exact_integer_divide_remainder_cast_shift_obligation,
    exact_integer_divide_remainder_then_shift_obligation,
    exact_integer_shift_cast_affine_obligation, exact_integer_shift_cast_shift_obligation,
    exact_integer_shift_then_arithmetic_chain_obligation,
};

pub(super) fn exact_integer_shift_obligation(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    if let Some(count) = known_integer_term_value(count_type, &count, semantic_axioms) {
        let count = match count {
            IntegerValue::Signed(count) => u128::try_from(count).ok(),
            IntegerValue::Unsigned(count) => Some(count),
        };
        return if count.is_some_and(|count| count < u128::from(value_type.bits())) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    let mut bounds = Vec::with_capacity(2);
    if count_type.minimum_value() != IntegerValue::Unsigned(0)
        && integer_value_cmp(count_type.minimum_value(), IntegerValue::Unsigned(0)).is_lt()
    {
        let zero = ScalarTerm::integer(
            count_type,
            match count_type.sign() {
                psi_core::IntegerSign::Signed => IntegerValue::Signed(0),
                psi_core::IntegerSign::Unsigned => IntegerValue::Unsigned(0),
            },
        )
        .expect("fixed integer count types admit zero");
        bounds.push(Proposition::LessOrEqual(zero, count.clone()));
    }
    let maximum = u128::from(value_type.bits() - 1);
    let maximum = match count_type.sign() {
        psi_core::IntegerSign::Signed => i128::try_from(maximum).ok().map(IntegerValue::Signed),
        psi_core::IntegerSign::Unsigned => Some(IntegerValue::Unsigned(maximum)),
    };
    if let Some(maximum) = maximum
        && count_type.admits(maximum)
        && integer_value_cmp(count_type.maximum_value(), maximum).is_gt()
    {
        let maximum = ScalarTerm::integer(count_type, maximum)
            .expect("admitted exact-shift maximum remains in the count type");
        bounds.push(Proposition::LessOrEqual(count, maximum));
    }
    canonical_conjunction(bounds)
}

pub(super) fn exact_integer_shift_left_obligation_with_context(
    proposition_context: &PropositionContext,
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    value: ScalarTerm,
    count: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    if let Some(count_value) = landed_exact_shift_count(
        value_type,
        count_type,
        &count,
        semantic_axioms,
        definition_axiom_count,
    ) {
        if let Some(obligation) = exact_integer_mixed_shift_chain_obligation(
            proposition_context,
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_shift_cast_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_affine_cast_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_divide_remainder_then_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_divide_remainder_cast_shift_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_cast_chain_then_shift_suffix_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_cast_then_mixed_shift_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_arithmetic_then_shift_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_cast_then_shift_left_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_shift_left_chain_obligation(
            value_type,
            value.clone(),
            count_value,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
    }
    if let Some(count) = exact_known_shift_count(value_type, count_type, &count, semantic_axioms) {
        let mut bounds = Vec::with_capacity(2);
        append_exact_shift_left_value_bounds(&mut bounds, value_type, value, count);
        return canonical_conjunction(bounds);
    }

    let count_bounds =
        exact_integer_shift_obligation(value_type, count_type, count.clone(), semantic_axioms);
    let mut bounds = match count_bounds {
        Proposition::Truth => Vec::new(),
        Proposition::Conjunction(bounds) => bounds,
        bound => vec![bound],
    };
    let known_maximum = known_shift_count_maximum(value_type, count_type, &count, semantic_axioms);
    if let Some(maximum) = known_maximum {
        bounds
            .retain(|bound| !matches!(bound, Proposition::LessOrEqual(left, _) if left == &count));
        let maximum = ScalarTerm::integer(
            count_type,
            match count_type.sign() {
                IntegerSign::Signed => IntegerValue::Signed(i128::from(maximum)),
                IntegerSign::Unsigned => IntegerValue::Unsigned(u128::from(maximum)),
            },
        )
        .expect("known exact-shift maximum remains in its count carrier");
        bounds.push(Proposition::LessOrEqual(count, maximum));
    }
    let maximum_count = known_maximum.unwrap_or_else(|| u32::from(value_type.bits() - 1));
    append_exact_shift_left_value_bounds(&mut bounds, value_type, value, maximum_count);
    canonical_conjunction(bounds)
}

#[cfg(test)]
pub(super) fn exact_integer_shift_left_obligation(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    value: ScalarTerm,
    count: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    exact_integer_shift_left_obligation_with_context(
        &PropositionContext::default(),
        value_type,
        count_type,
        value,
        count,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )
}
