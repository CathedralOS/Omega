//! Exact-shift and shift-chain reduction strategies.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::{
    ExactIntegerAffineOperation, IntegerOffset, canonical_conjunction,
    exact_integer_affine_preimage_interval, exact_integer_affine_preimage_obligation,
    exact_integer_carrier_total_hull_obligation, exact_integer_cast_chain_root_interval,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_divide_remainder_chain_hull, exact_integer_source_interval_obligation,
    fixed_integer_type_interval, integer_value_as_i128, integer_value_cmp,
    known_integer_term_value, landed_integer_constant_value, nonnegative_integer_factor,
    signed_negative_magnitude,
};

pub(super) fn exact_integer_shift_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    mut interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..=prior_axiom_count {
        if followed_definition
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
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
        let (nested_value, count_type, count) = match definition {
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
            _ => return None,
        };
        let count = landed_exact_shift_count(
            value_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        interval = match exact_integer_mixed_shift_preimage(value_type, interval, definition, count)
        {
            Ok(Some(interval)) => interval,
            Ok(None) => return Some(Proposition::Falsehood),
            Err(()) => return None,
        };
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
}

pub(super) fn exact_integer_mixed_shift_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let minimum = source_interval.0.max(target_interval.0);
    let maximum = source_interval.1.min(target_interval.1);
    if minimum > maximum {
        return Some(Proposition::Falsehood);
    }
    let mut interval = (minimum, maximum);
    let mut prior_axiom_count = semantic_axioms.len();
    let mut saw_left = false;
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
        let (nested_value, count_type, count) = match definition {
            ScalarTerm::ExactIntegerShiftLeft {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => {
                saw_left = true;
                (value, *count_type, count)
            }
            ScalarTerm::ExactIntegerShiftRight {
                value_type,
                count_type,
                value,
                count,
            } if *value_type == source_type => {
                saw_right = true;
                (value, *count_type, count)
            }
            _ => return None,
        };
        let count = landed_exact_shift_count(
            source_type,
            count_type,
            count,
            semantic_axioms,
            definition_index,
        )?;
        interval =
            match exact_integer_mixed_shift_preimage(source_type, interval, definition, count) {
                Ok(Some(interval)) => interval,
                Ok(None) => return Some(Proposition::Falsehood),
                Err(()) => return None,
            };
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        if saw_left
            && saw_right
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == source_type && machine_parameter_values.contains(id)
            )
        {
            return Some(exact_integer_source_interval_obligation(
                source_type,
                value,
                interval.0,
                interval.1,
            ));
        }
    }
    None
}

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

pub(super) fn exact_integer_shift_left_obligation(
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

pub(super) fn exact_integer_cast_then_mixed_shift_chain_obligation(
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

pub(super) fn exact_integer_cast_chain_then_shift_suffix_obligation(
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

pub(super) fn exact_integer_arithmetic_then_shift_chain_obligation(
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

pub(super) fn exact_integer_shift_then_arithmetic_chain_obligation(
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

pub(super) fn exact_integer_shift_cast_shift_obligation(
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

pub(super) fn exact_integer_affine_cast_shift_obligation(
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

pub(super) fn exact_integer_shift_cast_affine_obligation(
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

pub(super) fn exact_integer_divide_remainder_cast_shift_obligation(
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

pub(super) fn exact_integer_divide_remainder_then_shift_obligation(
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

pub(super) fn exact_integer_mixed_shift_chain_obligation(
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
            } if *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                let mapped = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(mapped)) => mapped,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                interval = mapped;
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            definition @ ScalarTerm::ExactIntegerShiftRight {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                let mapped = match exact_integer_mixed_shift_preimage(
                    value_type,
                    interval,
                    definition,
                    nested_count,
                ) {
                    Ok(Some(mapped)) => mapped,
                    Ok(None) => return Some(Proposition::Falsehood),
                    Err(()) => return None,
                };
                interval = mapped;
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
                saw_right = true;
            }
            _ => return None,
        }
        if saw_right
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return Some(exact_integer_source_interval_obligation(
                value_type, value, interval.0, interval.1,
            ));
        }
    }
    None
}

fn exact_integer_shift_left_input_interval(
    value_type: psi_core::IntegerType,
    count: u128,
) -> Option<(i128, i128)> {
    let count = u32::try_from(count).ok()?;
    if count >= u32::from(value_type.bits()) {
        return None;
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            Some((0, i128::try_from(maximum >> count).ok()?))
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            Some((minimum >> count, maximum >> count))
        }
    }
}

pub(super) fn exact_integer_mixed_shift_preimage(
    value_type: psi_core::IntegerType,
    interval: (i128, i128),
    definition: &ScalarTerm,
    count: u128,
) -> Result<Option<(i128, i128)>, ()> {
    let count = u32::try_from(count).map_err(|_| ())?;
    if count >= u32::from(value_type.bits()) {
        return Err(());
    }
    let scale = 1_i128.checked_shl(count).ok_or(())?;
    let mapped = match definition {
        ScalarTerm::ExactIntegerShiftLeft { .. } => {
            let minimum =
                interval.0.div_euclid(scale) + i128::from(interval.0.rem_euclid(scale) != 0);
            let maximum = interval.1.div_euclid(scale);
            (minimum, maximum)
        }
        ScalarTerm::ExactIntegerShiftRight { .. } => match value_type.sign() {
            IntegerSign::Signed => (
                interval.0.checked_mul(scale).ok_or(())?,
                interval
                    .1
                    .checked_add(1)
                    .ok_or(())?
                    .checked_mul(scale)
                    .ok_or(())?
                    .checked_sub(1)
                    .ok_or(())?,
            ),
            IntegerSign::Unsigned => {
                let minimum = u128::try_from(interval.0)
                    .map_err(|_| ())?
                    .checked_mul(scale as u128)
                    .ok_or(())?;
                let maximum = u128::try_from(interval.1)
                    .map_err(|_| ())?
                    .checked_add(1)
                    .ok_or(())?
                    .checked_mul(scale as u128)
                    .ok_or(())?
                    .checked_sub(1)
                    .ok_or(())?;
                (
                    i128::try_from(minimum).map_err(|_| ())?,
                    i128::try_from(maximum).map_err(|_| ())?,
                )
            }
        },
        _ => return Err(()),
    };
    let carrier_minimum = integer_value_as_i128(value_type.minimum_value()).ok_or(())?;
    let carrier_maximum = integer_value_as_i128(value_type.maximum_value()).ok_or(())?;
    let minimum = mapped.0.max(carrier_minimum);
    let maximum = mapped.1.min(carrier_maximum);
    Ok((minimum <= maximum).then_some((minimum, maximum)))
}

pub(super) fn exact_integer_cast_then_shift_left_chain_obligation(
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
    let mut cumulative_count = count;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
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
            ScalarTerm::ExactIntegerShiftLeft {
                value_type: nested_value_type,
                count_type,
                value: nested_value,
                count: nested_count,
            } if *nested_value_type == value_type => {
                let nested_count = landed_exact_shift_count(
                    value_type,
                    *count_type,
                    nested_count,
                    semantic_axioms,
                    definition_index,
                )?;
                let Some(total) = cumulative_count.checked_add(nested_count) else {
                    return Some(Proposition::Falsehood);
                };
                cumulative_count = total;
                value = (**nested_value).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type,
                operand,
            } if *target_type == value_type
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
                return Some(exact_integer_shifted_value_interval_obligation(
                    *source_type,
                    value_type,
                    (**operand).clone(),
                    cumulative_count,
                ));
            }
            _ => return None,
        }
    }
    None
}

fn exact_integer_shifted_value_interval_obligation(
    root_type: psi_core::IntegerType,
    value_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count == 0 {
        return Proposition::Truth;
    }
    if cumulative_count >= u128::from(value_type.bits()) {
        return exact_integer_source_interval_obligation(root_type, root, 0, 0);
    }
    let count = u32::try_from(cumulative_count).expect("count below native width fits u32");
    let (target_minimum, target_maximum) = match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum >> count).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            (minimum >> count, maximum >> count)
        }
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

pub(super) fn exact_integer_shift_left_chain_obligation(
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
    let mut cumulative_count = count;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
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
        let ScalarTerm::ExactIntegerShiftLeft {
            value_type: nested_value_type,
            count_type,
            value: nested_value,
            count: nested_count,
        } = definition
        else {
            break;
        };
        if *nested_value_type != value_type {
            break;
        }
        let Some(nested_count) = landed_exact_shift_count(
            value_type,
            *count_type,
            nested_count,
            semantic_axioms,
            definition_index,
        ) else {
            break;
        };
        let Some(total) = cumulative_count.checked_add(nested_count) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_count = total;
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == value_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_cumulative_shift_left_obligation(
        value_type,
        value,
        cumulative_count,
    ))
}

fn landed_exact_shift_count(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: &ScalarTerm,
    semantic_axioms: &[Proposition],
    prior_axiom_count: usize,
) -> Option<u128> {
    if count_type.is_address() || !matches!(count_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let count =
        landed_integer_constant_value(count_type, count, semantic_axioms, prior_axiom_count)?;
    let count = match count {
        IntegerValue::Signed(count) => u128::try_from(count).ok()?,
        IntegerValue::Unsigned(count) => count,
    };
    (count < u128::from(value_type.bits())).then_some(count)
}

pub(super) fn exact_integer_cumulative_shift_left_obligation(
    value_type: psi_core::IntegerType,
    value: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count == 0 {
        return Proposition::Truth;
    }
    if cumulative_count < u128::from(value_type.bits()) {
        let mut bounds = Vec::with_capacity(2);
        append_exact_shift_left_value_bounds(
            &mut bounds,
            value_type,
            value,
            u32::try_from(cumulative_count).expect("count below native width fits u32"),
        );
        return canonical_conjunction(bounds);
    }
    let zero = ScalarTerm::integer(
        value_type,
        match value_type.sign() {
            IntegerSign::Signed => IntegerValue::Signed(0),
            IntegerSign::Unsigned => IntegerValue::Unsigned(0),
        },
    )
    .expect("fixed integer value types admit zero");
    match value_type.sign() {
        IntegerSign::Unsigned => Proposition::LessOrEqual(value, zero),
        IntegerSign::Signed => canonical_conjunction(vec![
            Proposition::LessOrEqual(zero.clone(), value.clone()),
            Proposition::LessOrEqual(value, zero),
        ]),
    }
}

fn append_exact_shift_left_value_bounds(
    bounds: &mut Vec<Proposition>,
    value_type: psi_core::IntegerType,
    value: ScalarTerm,
    maximum_count: u32,
) {
    if maximum_count == 0 {
        return;
    }
    match value_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = value_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Unsigned(maximum >> maximum_count))
                    .expect("shifted unsigned maximum remains in its carrier");
            bounds.push(Proposition::LessOrEqual(value, maximum));
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (value_type.minimum_value(), value_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            let minimum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(minimum >> maximum_count))
                    .expect("shifted signed minimum remains in its carrier");
            let maximum =
                ScalarTerm::integer(value_type, IntegerValue::Signed(maximum >> maximum_count))
                    .expect("shifted signed maximum remains in its carrier");
            bounds.push(Proposition::LessOrEqual(minimum, value.clone()));
            bounds.push(Proposition::LessOrEqual(value, maximum));
        }
    }
}

fn exact_known_shift_count(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<u32> {
    let (known_type, known_count) = count.integer_value().or_else(|| {
        semantic_axioms.iter().rev().find_map(|axiom| {
            let Proposition::Equal(left, right) = axiom else {
                return None;
            };
            if left == count {
                right.integer_value()
            } else if right == count {
                left.integer_value()
            } else {
                None
            }
        })
    })?;
    if known_type != count_type || !count_type.admits(known_count) {
        return None;
    }
    let count = match known_count {
        IntegerValue::Unsigned(count) => u32::try_from(count).ok()?,
        IntegerValue::Signed(count) => u32::try_from(count).ok()?,
    };
    (count < u32::from(value_type.bits())).then_some(count)
}

fn known_shift_count_maximum(
    value_type: psi_core::IntegerType,
    count_type: psi_core::IntegerType,
    count: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<u32> {
    semantic_axioms
        .iter()
        .filter_map(|axiom| {
            let Proposition::LessOrEqual(left, right) = axiom else {
                return None;
            };
            if left != count {
                return None;
            }
            let (bound_type, bound) = right.integer_value()?;
            if bound_type != count_type || !count_type.admits(bound) {
                return None;
            }
            let bound = match bound {
                IntegerValue::Unsigned(bound) => u32::try_from(bound).ok()?,
                IntegerValue::Signed(bound) => u32::try_from(bound).ok()?,
            };
            (bound < u32::from(value_type.bits())).then_some(bound)
        })
        .min()
}

pub(super) fn exact_integer_shift_right_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut cumulative_count = 0_u128;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
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
        let ScalarTerm::ExactIntegerShiftRight {
            value_type,
            count_type,
            value: nested_value,
            count,
        } = definition
        else {
            break;
        };
        if *value_type != source_type {
            break;
        }
        let Some(count) = landed_exact_shift_count(
            source_type,
            *count_type,
            count,
            semantic_axioms,
            definition_index,
        ) else {
            break;
        };
        let Some(total) = cumulative_count.checked_add(count) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_count = total;
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_shift_right_chain_cast_interval_obligation(
        source_type,
        target_type,
        value,
        cumulative_count,
    ))
}

pub(super) fn exact_integer_shift_right_chain_cast_interval_obligation(
    root_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count >= u128::from(root_type.bits()) {
        return match (root_type.sign(), target_type.sign()) {
            (IntegerSign::Signed, IntegerSign::Unsigned) => {
                exact_integer_source_interval_obligation(root_type, root, 0, i128::MAX)
            }
            _ => Proposition::Truth,
        };
    }
    let count = u32::try_from(cumulative_count).expect("count below native width fits u32");
    let target_minimum = match target_type.minimum_value() {
        IntegerValue::Signed(minimum) => minimum.checked_shl(count),
        IntegerValue::Unsigned(_) => Some(0),
    };
    let target_maximum = match target_type.maximum_value() {
        IntegerValue::Signed(maximum) => u128::try_from(maximum).ok(),
        IntegerValue::Unsigned(maximum) => Some(maximum),
    }
    .and_then(|maximum| maximum.checked_add(1))
    .and_then(|exclusive| exclusive.checked_shl(count))
    .and_then(|exclusive| exclusive.checked_sub(1))
    .and_then(|maximum| i128::try_from(maximum).ok());
    let (Some(target_minimum), Some(target_maximum)) = (target_minimum, target_maximum) else {
        return Proposition::Falsehood;
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

pub(super) fn exact_integer_shift_left_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if source_type.is_address()
        || target_type.is_address()
        || !matches!(source_type.bits(), 8 | 16 | 32 | 64)
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
        || source_type == target_type
        || source_type.can_widen_to(target_type)
        || !source_type.can_exact_cast_to(target_type)
    {
        return None;
    }
    let mut cumulative_count = 0_u128;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_definition = false;
    for _ in 0..prior_axiom_count {
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
        let ScalarTerm::ExactIntegerShiftLeft {
            value_type,
            count_type,
            value: nested_value,
            count,
        } = definition
        else {
            break;
        };
        if *value_type != source_type {
            break;
        }
        let Some(count) = landed_exact_shift_count(
            source_type,
            *count_type,
            count,
            semantic_axioms,
            definition_index,
        ) else {
            break;
        };
        let Some(total) = cumulative_count.checked_add(count) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_count = total;
        value = (**nested_value).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_shift_chain_cast_interval_obligation(
        source_type,
        target_type,
        value,
        cumulative_count,
    ))
}

fn exact_integer_shift_chain_cast_interval_obligation(
    root_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_count: u128,
) -> Proposition {
    if cumulative_count >= u128::from(root_type.bits()) {
        return Proposition::Truth;
    }
    let count = u32::try_from(cumulative_count).expect("count below native width fits u32");
    let (target_minimum, target_maximum) = match target_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = target_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum >> count).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (target_type.minimum_value(), target_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            let Some(minimum) = signed_negative_magnitude(minimum.unsigned_abs() >> count) else {
                return Proposition::Falsehood;
            };
            (minimum, maximum >> count)
        }
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

pub(super) fn exact_integer_shifted_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    offset: IntegerOffset,
) -> Proposition {
    let translate_target_boundary = |boundary: i128| match offset {
        IntegerOffset::Nonnegative(magnitude) => {
            boundary.checked_sub(i128::try_from(magnitude).ok()?)
        }
        IntegerOffset::Negative(magnitude) => boundary.checked_add(i128::try_from(magnitude).ok()?),
    };
    let Some(target_minimum) =
        integer_value_as_i128(interval_type.minimum_value()).and_then(translate_target_boundary)
    else {
        return Proposition::Falsehood;
    };
    let Some(target_maximum) =
        integer_value_as_i128(interval_type.maximum_value()).and_then(translate_target_boundary)
    else {
        return Proposition::Falsehood;
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}
