//! Cross-family exact-shift and cast-composition reducers.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::super::{
    ExactIntegerAffineOperation, IntegerOffset, exact_integer_affine_preimage_interval,
    exact_integer_affine_preimage_obligation, exact_integer_carrier_total_hull_obligation,
    exact_integer_cast_chain_root_interval,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_divide_remainder_chain_hull, exact_integer_source_interval_obligation,
    fixed_integer_type_interval, landed_integer_constant_value, nonnegative_integer_factor,
};

use super::chains::{
    exact_integer_mixed_shift_preimage, exact_integer_shift_left_input_interval,
    landed_exact_shift_count,
};

pub(in crate::verification) fn exact_integer_cast_then_mixed_shift_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut saw_right = false;
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                saw_right |= matches!(definition, ScalarTerm::ExactIntegerShiftRight { .. });
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                interval = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(interval)) => interval,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } if saw_right
                && *target_type == value_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != value_type
                && !source_type.can_widen_to(value_type)
                && source_type.can_exact_cast_to(value_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                let source_interval = fixed_integer_type_interval(*source_type)?;
                let minimum = interval.0.max(source_interval.0);
                let maximum = interval.1.min(source_interval.1);
                if minimum > maximum {
                    return Some(Proposition::Falsehood);
                }
                return Some(exact_integer_source_interval_obligation(
                    *source_type,
                    (**operand).clone(),
                    minimum,
                    maximum,
                ));
            }
            _ => return None,
        }
    }
    None
}

pub(in crate::verification) fn exact_integer_cast_chain_then_shift_suffix_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if let Some((root_type, root, cast_interval)) = exact_integer_cast_chain_root_interval(
            value_type,
            value.clone(),
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        ) {
            let minimum = interval.0.max(cast_interval.0);
            let maximum = interval.1.min(cast_interval.1);
            return Some(if minimum <= maximum {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            } else {
                Proposition::Falsehood
            });
        }
        if let Some(obligation) = exact_integer_computed_prefix_conversion_interval_obligation(
            value_type,
            value.clone(),
            interval,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        ) {
            return Some(obligation);
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, nested_count) = match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            } if *nested_value_type == value_type => (value, *count_type, (definition, count)),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            value_type,
            count_type,
            nested_count.1,
            semantic_axioms,
            definition_index,
        )?;
        interval =
            match exact_integer_mixed_shift_preimage(value_type, interval, nested_count.0, count) {
                Ok(Some(interval)) => interval,
                Ok(None) => return Some(Proposition::Falsehood),
                Err(()) => return None,
            };
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
    }
    None
}

pub(in crate::verification) fn exact_integer_arithmetic_then_shift_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let Some(mut interval) = exact_integer_shift_left_input_interval(value_type, count) else {
        return Some(Proposition::Falsehood);
    };
    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut saw_arithmetic = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if saw_arithmetic
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return match exact_integer_affine_preimage_obligation(
                value_type,
                value,
                coefficient,
                offset,
                interval,
            ) {
                Ok(obligation) => Some(obligation),
                Err(()) => None,
            };
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if !saw_arithmetic && *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                interval = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(interval)) => interval,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => {
                if landed_integer_constant_value(
                    value_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(value_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact arithmetic definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
                saw_arithmetic = true;
            }
            _ => return None,
        }
    }
    None
}

pub(in crate::verification) fn exact_integer_shift_then_arithmetic_chain_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(value_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let carrier_interval = fixed_integer_type_interval(value_type)?;
    let mut shifted_interval: Option<(i128, i128)> = None;
    let mut constant_decision = None;
    let mut mathematical_empty = false;
    let mut saw_shift = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if saw_shift
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            if let Some(decision) = constant_decision {
                return Some(decision);
            }
            if mathematical_empty {
                return Some(Proposition::Falsehood);
            }
            let interval = shifted_interval?;
            return Some(exact_integer_source_interval_obligation(
                value_type, value, interval.0, interval.1,
            ));
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if !saw_shift && *scalar_type == value_type => {
                if landed_integer_constant_value(
                    value_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(value_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact arithmetic definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
            }
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count,
            } if *nested_value_type == value_type => {
                if !saw_shift {
                    if coefficient == 0 {
                        let decision = if offset.is_representable(value_type) {
                            Proposition::Truth
                        } else {
                            Proposition::Falsehood
                        };
                        constant_decision = Some(decision);
                    } else {
                        shifted_interval = match exact_integer_affine_preimage_interval(
                            value_type,
                            coefficient,
                            offset,
                            carrier_interval,
                        ) {
                            Ok(Some(interval)) => Some(interval),
                            Ok(None) => {
                                mathematical_empty = true;
                                None
                            }
                            Err(()) => return None,
                        };
                    }
                    saw_shift = true;
                }
                let count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if constant_decision.is_none() && !mathematical_empty {
                    shifted_interval = match exact_integer_mixed_shift_preimage(
                        value_type,
                        shifted_interval?,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => Some(interval),
                        Ok(None) => {
                            mathematical_empty = true;
                            None
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            _ => return None,
        }
    }
    None
}

pub(in crate::verification) fn exact_integer_shift_cast_shift_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(target_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value: nested_value,
                count,
            } if *value_type == target_type => {
                let count = landed_exact_shift_count(
                    target_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if !mathematical_empty {
                    interval = match exact_integer_mixed_shift_preimage(
                        target_type,
                        interval,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            mathematical_empty = true;
                            interval
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                if !mathematical_empty {
                    let source_interval = fixed_integer_type_interval(*source_type)?;
                    let minimum = interval.0.max(source_interval.0);
                    let maximum = interval.1.min(source_interval.1);
                    if minimum > maximum {
                        mathematical_empty = true;
                    } else {
                        interval = (minimum, maximum);
                    }
                }
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            return Some(if mathematical_empty {
                Proposition::Falsehood
            } else {
                exact_integer_source_interval_obligation(
                    source_type,
                    source_value,
                    interval.0,
                    interval.1,
                )
            });
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => (value, *count_type, count),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            source_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        if !mathematical_empty {
            interval = match exact_integer_mixed_shift_preimage(
                source_type,
                interval,
                definition,
                count,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    mathematical_empty = true;
                    interval
                }
                Err(()) => return None,
            };
        }
        source_value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

pub(in crate::verification) fn exact_integer_affine_cast_shift_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(target_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value: nested_value,
                count,
            } if *value_type == target_type => {
                let count = landed_exact_shift_count(
                    target_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if !mathematical_empty {
                    interval = match exact_integer_mixed_shift_preimage(
                        target_type,
                        interval,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            mathematical_empty = true;
                            interval
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                if !mathematical_empty {
                    let source_interval = fixed_integer_type_interval(*source_type)?;
                    let minimum = interval.0.max(source_interval.0);
                    let maximum = interval.1.min(source_interval.1);
                    if minimum > maximum {
                        mathematical_empty = true;
                    } else {
                        interval = (minimum, maximum);
                    }
                }
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            if mathematical_empty {
                return Some(Proposition::Falsehood);
            }
            return exact_integer_affine_preimage_obligation(
                source_type,
                source_value,
                coefficient,
                offset,
                interval,
            )
            .ok();
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_value => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    source_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == source_type => (
                left,
                right,
                nonnegative_integer_factor(
                    source_type,
                    landed_integer_constant_value(
                        source_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(source_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(source_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        source_value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

pub(in crate::verification) fn exact_integer_shift_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(target_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, mut source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            }
            | ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => {
                if landed_integer_constant_value(
                    target_type,
                    left,
                    semantic_axioms,
                    definition_index,
                )
                .is_some()
                {
                    return None;
                }
                let constant = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let (nested_coefficient, nested_offset) = match definition {
                    ScalarTerm::ExactIntegerAdd { .. } => (1, IntegerOffset::from_value(constant)),
                    ScalarTerm::ExactIntegerSubtract { .. } => {
                        (1, IntegerOffset::from_subtrahend(constant))
                    }
                    ScalarTerm::ExactIntegerMultiply { .. } => (
                        nonnegative_integer_factor(target_type, constant)?,
                        IntegerOffset::Nonnegative(0),
                    ),
                    _ => unreachable!("matched one exact affine definition"),
                };
                offset = nested_offset
                    .checked_multiply(coefficient)
                    .and_then(|nested| nested.checked_add(offset))?;
                coefficient = coefficient.checked_mul(nested_coefficient)?;
                value = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };

    let target_carrier = fixed_integer_type_interval(target_type)?;
    let mut constant_decision = None;
    let mut mathematical_empty = false;
    let mut interval = if coefficient == 0 {
        constant_decision = Some(if offset.is_representable(target_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
        target_carrier
    } else {
        match exact_integer_affine_preimage_interval(
            target_type,
            coefficient,
            offset,
            target_carrier,
        ) {
            Ok(Some(interval)) => interval,
            Ok(None) => {
                mathematical_empty = true;
                target_carrier
            }
            Err(()) => return None,
        }
    };
    if constant_decision.is_none() && !mathematical_empty {
        let source_carrier = fixed_integer_type_interval(source_type)?;
        let minimum = interval.0.max(source_carrier.0);
        let maximum = interval.1.min(source_carrier.1);
        if minimum > maximum {
            mathematical_empty = true;
        } else {
            interval = (minimum, maximum);
        }
    }

    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            if let Some(decision) = constant_decision {
                return Some(decision);
            }
            if mathematical_empty {
                return Some(Proposition::Falsehood);
            }
            return Some(exact_integer_source_interval_obligation(
                source_type,
                source_value,
                interval.0,
                interval.1,
            ));
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_value => Some((index, right)),
                _ => None,
            })?;
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => (value, *count_type, count),
            _ => return None,
        };
        let count = landed_exact_shift_count(
            source_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        if constant_decision.is_none() && !mathematical_empty {
            interval = match exact_integer_mixed_shift_preimage(
                source_type,
                interval,
                definition,
                count,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    mathematical_empty = true;
                    interval
                }
                Err(()) => return None,
            };
        }
        source_value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

pub(in crate::verification) fn exact_integer_divide_remainder_cast_shift_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(target_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let (source_type, source_value) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        match definition {
            definition @ ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value: nested_value,
                count,
            }
            | definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value: nested_value,
                count,
            } if *value_type == target_type => {
                let count = landed_exact_shift_count(
                    target_type,
                    *count_type,
                    count,
                    semantic_axioms,
                    definition_index,
                )?;
                if !mathematical_empty {
                    interval = match exact_integer_mixed_shift_preimage(
                        target_type,
                        interval,
                        definition,
                        count,
                    ) {
                        Ok(Some(interval)) => interval,
                        Ok(None) => {
                            mathematical_empty = true;
                            interval
                        }
                        Err(()) => return None,
                    };
                }
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type) =>
            {
                prior_axiom_count = definition_index;
                break (*source_type, (**operand).clone());
            }
            _ => return None,
        }
    };
    let hull = exact_integer_divide_remainder_chain_hull(
        source_type,
        source_value,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )?;
    let target_carrier = fixed_integer_type_interval(target_type)?;
    if hull.0 < target_carrier.0 || hull.1 > target_carrier.1 {
        return None;
    }
    if mathematical_empty {
        return Some(Proposition::Falsehood);
    }
    exact_integer_carrier_total_hull_obligation(hull, interval)
}

pub(in crate::verification) fn exact_integer_divide_remainder_then_shift_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    count: u128,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.is_address() || !matches!(value_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut interval = exact_integer_shift_left_input_interval(value_type, count)?;
    let mut mathematical_empty = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (nested_value, count_type, nested_count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            }
            | ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value,
                count,
            } if *nested_value_type == value_type => (value, *count_type, count),
            _ => break,
        };
        let nested_count = landed_exact_shift_count(
            value_type,
            count_type,
            nested_count,
            semantic_axioms,
            definition_index,
        )?;
        if !mathematical_empty {
            interval = match exact_integer_mixed_shift_preimage(
                value_type,
                interval,
                definition,
                nested_count,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => {
                    mathematical_empty = true;
                    interval
                }
                Err(()) => return None,
            };
        }
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
    }
    let hull = exact_integer_divide_remainder_chain_hull(
        value_type,
        value,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )?;
    if mathematical_empty {
        return Some(Proposition::Falsehood);
    }
    exact_integer_carrier_total_hull_obligation(hull, interval)
}
