//! Exact conversion-chain and interval foundations.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::super::integer_affine::{integer_offset_ceil_div, integer_offset_floor_div};
use super::super::integer_shift::exact_integer_shift_prefix_interval_obligation;
use super::super::{
    IntegerOffset, checked_integer_ceil_division, checked_integer_floor_division,
    checked_signed_integer_product, exact_integer_carrier_total_hull_obligation,
    exact_integer_source_interval_obligation, fixed_integer_type_interval,
    landed_integer_constant_value, nonnegative_integer_factor, signed_negative_magnitude,
};

use super::composition::exact_integer_divide_remainder_chain_hull;

pub(in crate::verification) fn exact_integer_cast_chain_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut interval = (
        source_interval.0.max(target_interval.0),
        source_interval.1.min(target_interval.1),
    );
    let mut expected_target = source_type;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_nested_cast = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::IntegerExactCast {
            source_type: nested_source,
            target_type: nested_target,
            operand: nested_operand,
        } = definition
        else {
            return None;
        };
        if *nested_target != expected_target
            || !partial_fixed_native_integer_cast(*nested_source, *nested_target)
        {
            return None;
        }
        let nested_interval = fixed_integer_type_interval(*nested_source)?;
        interval.0 = interval.0.max(nested_interval.0);
        interval.1 = interval.1.min(nested_interval.1);
        operand = (**nested_operand).clone();
        expected_target = *nested_source;
        prior_axiom_count = definition_index;
        followed_nested_cast = true;
    }
    if !followed_nested_cast
        || !matches!(
            &operand,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == expected_target && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    if interval.0 > interval.1 {
        return Some(Proposition::Falsehood);
    }
    Some(exact_integer_source_interval_obligation(
        expected_target,
        operand,
        interval.0,
        interval.1,
    ))
}

pub(in crate::verification) fn exact_integer_computed_prefix_cast_chain_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut interval = (
        source_interval.0.max(target_interval.0),
        source_interval.1.min(target_interval.1),
    );
    let mut expected_target = source_type;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut followed_nested_cast = false;
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let ScalarTerm::IntegerExactCast {
            source_type: nested_source,
            target_type: nested_target,
            operand: nested_operand,
        } = definition
        else {
            break;
        };
        if *nested_target != expected_target
            || !partial_fixed_native_integer_cast(*nested_source, *nested_target)
        {
            return None;
        }
        let nested_interval = fixed_integer_type_interval(*nested_source)?;
        interval.0 = interval.0.max(nested_interval.0);
        interval.1 = interval.1.min(nested_interval.1);
        operand = (**nested_operand).clone();
        expected_target = *nested_source;
        prior_axiom_count = definition_index;
        followed_nested_cast = true;
    }
    if !followed_nested_cast {
        return None;
    }
    if interval.0 > interval.1 {
        return Some(Proposition::Falsehood);
    }
    let definition =
        semantic_axioms[..prior_axiom_count]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some(right),
                _ => None,
            })?;
    match definition {
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. } => {
            let hull = exact_integer_divide_remainder_chain_hull(
                expected_target,
                operand,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )?;
            (hull.0 >= interval.0 && hull.1 <= interval.1).then_some(Proposition::Truth)
        }
        ScalarTerm::ExactIntegerShiftLeft { .. } | ScalarTerm::ExactIntegerShiftRight { .. } => {
            exact_integer_shift_prefix_interval_obligation(
                expected_target,
                operand,
                interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerAdd { .. } | ScalarTerm::ExactIntegerSubtract { .. } => {
            exact_integer_affine_prefix_interval_obligation(
                expected_target,
                operand,
                interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerMultiply { .. } => exact_integer_affine_prefix_interval_obligation(
            expected_target,
            operand.clone(),
            interval,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        )
        .or_else(|| {
            exact_integer_signed_product_prefix_interval_obligation(
                expected_target,
                operand,
                interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        }),
        _ => None,
    }
}

pub(in crate::verification) fn exact_integer_computed_prefix_cast_chain_interval_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut cast_interval = fixed_integer_type_interval(target_type)?;
    let mut expected_target = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut cast_count = 0_usize;
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
        let ScalarTerm::IntegerExactCast {
            source_type,
            target_type: cast_target,
            operand,
        } = definition
        else {
            break;
        };
        if *cast_target != expected_target
            || !partial_fixed_native_integer_cast(*source_type, *cast_target)
        {
            return None;
        }
        let source_interval = fixed_integer_type_interval(*source_type)?;
        cast_interval.0 = cast_interval.0.max(source_interval.0);
        cast_interval.1 = cast_interval.1.min(source_interval.1);
        value = (**operand).clone();
        expected_target = *source_type;
        prior_axiom_count = definition_index;
        cast_count += 1;
    }
    if cast_count < 2 {
        return None;
    }
    let interval = (
        requested_interval.0.max(cast_interval.0),
        requested_interval.1.min(cast_interval.1),
    );
    let definition =
        semantic_axioms[..prior_axiom_count]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == &value => Some(right),
                _ => None,
            })?;
    if matches!(
        definition,
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. }
    ) {
        let hull = exact_integer_divide_remainder_chain_hull(
            expected_target,
            value,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        )?;
        if hull.0 < cast_interval.0 || hull.1 > cast_interval.1 {
            return None;
        }
        return exact_integer_carrier_total_hull_obligation(hull, interval);
    }
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        value,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

fn exact_integer_computed_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    value: ScalarTerm,
    interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let definition = semantic_axioms[..definition_axiom_count.min(semantic_axioms.len())]
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == &value => Some(right),
            _ => None,
        })?;
    match definition {
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. } => {
            let hull = exact_integer_divide_remainder_chain_hull(
                value_type,
                value,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )?;
            exact_integer_carrier_total_hull_obligation(hull, interval)
        }
        ScalarTerm::ExactIntegerShiftLeft { .. } | ScalarTerm::ExactIntegerShiftRight { .. } => {
            exact_integer_shift_prefix_interval_obligation(
                value_type,
                value,
                interval,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerAdd { .. } | ScalarTerm::ExactIntegerSubtract { .. } => {
            exact_integer_affine_prefix_interval_obligation(
                value_type,
                value,
                interval,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        ScalarTerm::ExactIntegerMultiply { .. } => exact_integer_affine_prefix_interval_obligation(
            value_type,
            value.clone(),
            interval,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
        .or_else(|| {
            exact_integer_signed_product_prefix_interval_obligation(
                value_type,
                value,
                interval,
                semantic_axioms,
                definition_axiom_count,
                machine_parameter_values,
            )
        }),
        _ => None,
    }
}

pub(in crate::verification) fn exact_integer_computed_prefix_widen_chain_interval_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut expected_target = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut widen_count = 0_usize;
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
        let ScalarTerm::IntegerWiden {
            source_type,
            target_type: widen_target,
            operand,
        } = definition
        else {
            break;
        };
        if *widen_target != expected_target || !source_type.can_widen_to(*widen_target) {
            return None;
        }
        value = (**operand).clone();
        expected_target = *source_type;
        prior_axiom_count = definition_index;
        widen_count += 1;
    }
    if widen_count == 0 {
        return None;
    }
    let source_interval = fixed_integer_type_interval(expected_target)?;
    let interval = (
        requested_interval.0.max(source_interval.0),
        requested_interval.1.min(source_interval.1),
    );
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        value,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

pub(in crate::verification) fn exact_integer_computed_prefix_mixed_conversion_chain_cast_obligation(
    source_type: psi_core::IntegerType,
    target_type: psi_core::IntegerType,
    mut operand: ScalarTerm,
    semantic_axioms: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if !partial_fixed_native_integer_cast(source_type, target_type) {
        return None;
    }
    let source_interval = fixed_integer_type_interval(source_type)?;
    let target_interval = fixed_integer_type_interval(target_type)?;
    let mut interval = (
        source_interval.0.max(target_interval.0),
        source_interval.1.min(target_interval.1),
    );
    let mut expected_target = source_type;
    let mut prior_axiom_count = semantic_axioms.len();
    let mut saw_widen = false;
    for _ in 0..=prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (nested_source, nested_operand) = match definition {
            ScalarTerm::IntegerWiden {
                source_type: nested_source,
                target_type: nested_target,
                operand: nested_operand,
            } if *nested_target == expected_target
                && nested_source.can_widen_to(*nested_target) =>
            {
                saw_widen = true;
                (*nested_source, nested_operand)
            }
            ScalarTerm::IntegerExactCast {
                source_type: nested_source,
                target_type: nested_target,
                operand: nested_operand,
            } if *nested_target == expected_target
                && partial_fixed_native_integer_cast(*nested_source, *nested_target) =>
            {
                (*nested_source, nested_operand)
            }
            ScalarTerm::IntegerWiden { .. } | ScalarTerm::IntegerExactCast { .. } => return None,
            _ => break,
        };
        let nested_interval = fixed_integer_type_interval(nested_source)?;
        interval.0 = interval.0.max(nested_interval.0);
        interval.1 = interval.1.min(nested_interval.1);
        operand = (**nested_operand).clone();
        expected_target = nested_source;
        prior_axiom_count = definition_index;
    }
    if !saw_widen {
        return None;
    }
    let definition =
        semantic_axioms[..prior_axiom_count]
            .iter()
            .rev()
            .find_map(|axiom| match axiom {
                Proposition::Equal(left, right) if left == &operand => Some(right),
                _ => None,
            })?;
    if matches!(
        definition,
        ScalarTerm::ExactIntegerDivide { .. } | ScalarTerm::ExactIntegerRemainder { .. }
    ) {
        let hull = exact_integer_divide_remainder_chain_hull(
            expected_target,
            operand,
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        )?;
        return (hull.0 >= interval.0 && hull.1 <= interval.1).then_some(Proposition::Truth);
    }
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        operand,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

pub(in crate::verification) fn exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut interval = fixed_integer_type_interval(target_type)?;
    let mut expected_target = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut saw_widen = false;
    let mut saw_cast = false;
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
        let (source_type, operand) = match definition {
            ScalarTerm::IntegerWiden {
                source_type,
                target_type: conversion_target,
                operand,
            } if *conversion_target == expected_target
                && source_type.can_widen_to(*conversion_target) =>
            {
                saw_widen = true;
                (*source_type, operand)
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: conversion_target,
                operand,
            } if *conversion_target == expected_target
                && partial_fixed_native_integer_cast(*source_type, *conversion_target) =>
            {
                saw_cast = true;
                (*source_type, operand)
            }
            ScalarTerm::IntegerWiden { .. } | ScalarTerm::IntegerExactCast { .. } => return None,
            _ => break,
        };
        let source_interval = fixed_integer_type_interval(source_type)?;
        interval.0 = interval.0.max(source_interval.0);
        interval.1 = interval.1.min(source_interval.1);
        value = (**operand).clone();
        expected_target = source_type;
        prior_axiom_count = definition_index;
    }
    if !saw_widen || !saw_cast {
        return None;
    }
    let interval = (
        requested_interval.0.max(interval.0),
        requested_interval.1.min(interval.1),
    );
    exact_integer_computed_prefix_interval_obligation(
        expected_target,
        value,
        interval,
        semantic_axioms,
        prior_axiom_count,
        machine_parameter_values,
    )
}

pub(in crate::verification) fn exact_integer_computed_prefix_conversion_interval_obligation(
    target_type: psi_core::IntegerType,
    value: ScalarTerm,
    requested_interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    exact_integer_computed_prefix_cast_chain_interval_obligation(
        target_type,
        value.clone(),
        requested_interval,
        semantic_axioms,
        definition_axiom_count,
        machine_parameter_values,
    )
    .or_else(|| {
        exact_integer_computed_prefix_widen_chain_interval_obligation(
            target_type,
            value.clone(),
            requested_interval,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    })
    .or_else(|| {
        exact_integer_computed_prefix_mixed_conversion_chain_interval_obligation(
            target_type,
            value,
            requested_interval,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    })
}

fn exact_integer_affine_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut coefficient = 1_u128;
    let mut offset = IntegerOffset::Nonnegative(0);
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
            return exact_integer_affine_preimage_obligation(
                value_type,
                value,
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
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    value_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == value_type => (
                left,
                right,
                nonnegative_integer_factor(
                    value_type,
                    landed_integer_constant_value(
                        value_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(value_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(value_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
}

fn exact_integer_signed_product_prefix_interval_obligation(
    value_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    interval: (i128, i128),
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if value_type.sign() != IntegerSign::Signed {
        return None;
    }
    let mut product = Some(IntegerOffset::Nonnegative(1));
    let mut saw_negative = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
    for _ in 0..=prior_axiom_count {
        if followed_definition
            && saw_negative
            && matches!(
                &value,
                ScalarTerm::Value {
                    id,
                    scalar_type: ScalarType::Integer(root_type),
                } if *root_type == value_type && machine_parameter_values.contains(id)
            )
        {
            return exact_integer_signed_product_interval_preimage_obligation(
                value_type, value, product?, interval,
            );
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            return None;
        };
        if *scalar_type != value_type
            || landed_integer_constant_value(value_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            return None;
        }
        let factor =
            landed_integer_constant_value(value_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(factor_value) = factor else {
            return None;
        };
        product = checked_signed_integer_product(product, factor);
        saw_negative |= factor_value < 0;
        value = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    None
}

fn exact_integer_signed_product_interval_preimage_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    product: IntegerOffset,
    interval: (i128, i128),
) -> Option<Proposition> {
    if product.magnitude() == 0 {
        return Some(if interval.0 <= 0 && 0 <= interval.1 {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let magnitude = product.magnitude();
    let (minimum, maximum) = if magnitude > i128::MAX as u128 {
        (0, 0)
    } else {
        let magnitude = i128::try_from(magnitude).ok()?;
        let signed_product = match product {
            IntegerOffset::Nonnegative(_) => magnitude,
            IntegerOffset::Negative(_) => magnitude.checked_neg()?,
        };
        if signed_product > 0 {
            (
                checked_integer_ceil_division(interval.0, signed_product)?,
                checked_integer_floor_division(interval.1, signed_product)?,
            )
        } else {
            (
                checked_integer_ceil_division(interval.1, signed_product)?,
                checked_integer_floor_division(interval.0, signed_product)?,
            )
        }
    };
    Some(exact_integer_source_interval_obligation(
        root_type, root, minimum, maximum,
    ))
}

pub(in crate::verification) fn partial_fixed_native_integer_cast(
    source: psi_core::IntegerType,
    target: psi_core::IntegerType,
) -> bool {
    fixed_integer_type_interval(source).is_some()
        && fixed_integer_type_interval(target).is_some()
        && source != target
        && source.can_exact_cast_to(target)
        && !source.can_widen_to(target)
}

pub(in crate::verification) fn exact_integer_cast_chain_root_interval(
    target_type: psi_core::IntegerType,
    mut value: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<(psi_core::IntegerType, ScalarTerm, (i128, i128))> {
    let mut interval = fixed_integer_type_interval(target_type)?;
    let mut expected_target_type = target_type;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut cast_count = 0_usize;
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &value => Some((index, right)),
                _ => None,
            })?;
        let ScalarTerm::IntegerExactCast {
            source_type,
            target_type: cast_target_type,
            operand,
        } = definition
        else {
            return None;
        };
        if *cast_target_type != expected_target_type
            || !partial_fixed_native_integer_cast(*source_type, *cast_target_type)
        {
            return None;
        }
        let source_interval = fixed_integer_type_interval(*source_type)?;
        interval.0 = interval.0.max(source_interval.0);
        interval.1 = interval.1.min(source_interval.1);
        cast_count += 1;
        value = (**operand).clone();
        expected_target_type = *source_type;
        prior_axiom_count = definition_index;
        if matches!(
            &value,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if cast_count >= 2
                && *root_type == expected_target_type
                && machine_parameter_values.contains(id)
        ) {
            return Some((expected_target_type, value, interval));
        }
    }
    None
}

pub(in crate::verification) fn exact_integer_affine_preimage_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: u128,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Result<Proposition, ()> {
    let offset_as_i128 = |offset| match offset {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).ok(),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => None,
        IntegerOffset::Negative(value) => signed_negative_magnitude(value),
    };
    if coefficient == 0 {
        let Some(constant) = offset_as_i128(offset) else {
            return Ok(Proposition::Falsehood);
        };
        return Ok(if interval.0 <= constant && constant <= interval.1 {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let Some(interval) =
        exact_integer_affine_preimage_interval(root_type, coefficient, offset, interval)?
    else {
        return Ok(Proposition::Falsehood);
    };
    Ok(exact_integer_source_interval_obligation(
        root_type, root, interval.0, interval.1,
    ))
}

pub(in crate::verification) fn exact_integer_affine_preimage_interval(
    input_type: psi_core::IntegerType,
    coefficient: u128,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Result<Option<(i128, i128)>, ()> {
    debug_assert_ne!(coefficient, 0);
    let lower_numerator = IntegerOffset::from_value(IntegerValue::Signed(interval.0))
        .checked_add(offset.negated())
        .ok_or(())?;
    let upper_numerator = IntegerOffset::from_value(IntegerValue::Signed(interval.1))
        .checked_add(offset.negated())
        .ok_or(())?;
    let lower = integer_offset_ceil_div(lower_numerator, coefficient);
    let upper = integer_offset_floor_div(upper_numerator, coefficient);
    let lower = match lower {
        IntegerOffset::Nonnegative(value) => match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => return Ok(None),
        },
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => i128::MIN,
        IntegerOffset::Negative(value) => signed_negative_magnitude(value).ok_or(())?,
    };
    let upper = match upper {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).unwrap_or(i128::MAX),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => {
            return Ok(None);
        }
        IntegerOffset::Negative(value) => signed_negative_magnitude(value).ok_or(())?,
    };
    let carrier = fixed_integer_type_interval(input_type).ok_or(())?;
    let lower = lower.max(carrier.0);
    let upper = upper.min(carrier.1);
    Ok((lower <= upper).then_some((lower, upper)))
}
