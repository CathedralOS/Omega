//! Exact-cast reduction orchestration.
//!
//! Conversion-chain foundations and cross-family reducers remain explicit
//! trusted migration dependencies until they prove canonical ledger goals.
//! This parent owns precedence and the direct fallback, not their algorithms.

use std::collections::BTreeSet;

use psi_core::{Proposition, ScalarTerm, ScalarType, ValueId};

use super::integer_shift::{
    exact_integer_mixed_shift_chain_cast_obligation,
    exact_integer_shift_left_chain_cast_obligation,
    exact_integer_shift_right_chain_cast_obligation,
};
use super::integer_value_cmp;

mod chains;
mod composition;

pub(super) use chains::{
    exact_integer_affine_preimage_interval, exact_integer_affine_preimage_obligation,
    exact_integer_cast_chain_obligation, exact_integer_cast_chain_root_interval,
    exact_integer_computed_prefix_cast_chain_obligation,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation,
    partial_fixed_native_integer_cast,
};
#[cfg(test)]
pub(super) use chains::{
    exact_integer_computed_prefix_cast_chain_interval_obligation,
    exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation,
    exact_integer_computed_prefix_widen_chain_interval_obligation,
};
pub(super) use composition::{
    exact_integer_affine_chain_cast_obligation, exact_integer_cast_then_offset_obligation,
    exact_integer_divide_remainder_cast_affine_obligation,
    exact_integer_divide_remainder_chain_hull,
    exact_integer_divide_remainder_then_affine_obligation,
    exact_integer_signed_affine_chain_cast_obligation,
    exact_integer_signed_multiply_chain_cast_obligation,
    exact_integer_signed_product_interval_obligation,
};
use composition::{
    exact_integer_divide_remainder_chain_cast_obligation,
    exact_integer_multiply_chain_cast_obligation, exact_integer_offset_chain_cast_obligation,
};

pub(super) fn exact_integer_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    if let Some(obligation) = exact_integer_cast_chain_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_computed_prefix_cast_chain_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_divide_remainder_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_mixed_shift_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_shift_right_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_shift_left_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_affine_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_multiply_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_signed_multiply_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_signed_affine_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    if let Some(obligation) = exact_integer_offset_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    let roundtrip = {
        let mut current = &operand;
        let mut expected_widened_type = source_type;
        let mut prior_axiom_count = semantic_axioms.len();
        let mut established = false;
        for _ in 0..semantic_axioms.len() {
            let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
                .iter()
                .enumerate()
                .rev()
                .find_map(|(index, axiom)| match axiom {
                    Proposition::Equal(left, right) if left == current => Some((index, right)),
                    _ => None,
                })
            else {
                break;
            };
            let ScalarTerm::IntegerWiden {
                source_type: original_type,
                target_type: widened_type,
                operand: original_operand,
            } = definition
            else {
                break;
            };
            let ScalarTerm::Value {
                id: original_value,
                scalar_type: ScalarType::Integer(original_operand_type),
            } = original_operand.as_ref()
            else {
                break;
            };
            if *widened_type != expected_widened_type
                || *original_operand_type != *original_type
                || !original_type.can_widen_to(*widened_type)
            {
                break;
            }
            if *original_type == target_type {
                established = machine_parameter_values.contains(original_value);
                break;
            }
            current = original_operand;
            expected_widened_type = *original_type;
            prior_axiom_count = definition_index;
        }
        established
    };
    if roundtrip {
        return Proposition::Truth;
    }
    if let Some(obligation) = exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
        source_type,
        target_type,
        operand.clone(),
        semantic_axioms,
        machine_parameter_values,
    ) {
        return obligation;
    }
    let mut bounds = Vec::with_capacity(2);
    let source_minimum = source_type.minimum_value();
    let target_minimum = target_type.minimum_value();
    if integer_value_cmp(target_minimum, source_minimum).is_gt() {
        let boundary = target_type
            .exact_cast_value_to(source_type, target_minimum)
            .expect("a stricter target minimum is representable by the source type");
        let boundary = ScalarTerm::integer(source_type, boundary)
            .expect("converted exact-cast minimum is admitted by its source type");
        bounds.push(Proposition::LessOrEqual(boundary, operand.clone()));
    }

    let source_maximum = source_type.maximum_value();
    let target_maximum = target_type.maximum_value();
    if integer_value_cmp(target_maximum, source_maximum).is_lt() {
        let boundary = target_type
            .exact_cast_value_to(source_type, target_maximum)
            .expect("a stricter target maximum is representable by the source type");
        let boundary = ScalarTerm::integer(source_type, boundary)
            .expect("converted exact-cast maximum is admitted by its source type");
        bounds.push(Proposition::LessOrEqual(operand, boundary));
    }

    match bounds.len() {
        0 => unreachable!("validator rejects exact casts whose source range already fits"),
        1 => bounds.pop().expect("one exact-cast bound exists"),
        _ => Proposition::Conjunction(bounds),
    }
}
