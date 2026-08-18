//! Shared exact-shift chain foundations and direct-chain reducers.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::super::{
    IntegerOffset, canonical_conjunction, exact_integer_source_interval_obligation,
    fixed_integer_type_interval, integer_value_as_i128, landed_integer_constant_value,
    signed_negative_magnitude,
};

pub(in crate::verification) fn exact_integer_shift_prefix_interval_obligation(
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

pub(in crate::verification) fn exact_integer_mixed_shift_chain_cast_obligation(
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

pub(in crate::verification) fn exact_integer_mixed_shift_chain_obligation(
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

pub(super) fn exact_integer_shift_left_input_interval(
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

pub(in crate::verification) fn exact_integer_mixed_shift_preimage(
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

pub(in crate::verification) fn exact_integer_cast_then_shift_left_chain_obligation(
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

pub(in crate::verification) fn exact_integer_shift_left_chain_obligation(
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

pub(super) fn landed_exact_shift_count(
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

pub(in crate::verification) fn exact_integer_cumulative_shift_left_obligation(
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

pub(super) fn append_exact_shift_left_value_bounds(
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

pub(super) fn exact_known_shift_count(
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

pub(super) fn known_shift_count_maximum(
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

pub(in crate::verification) fn exact_integer_shift_right_chain_cast_obligation(
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

pub(in crate::verification) fn exact_integer_shift_right_chain_cast_interval_obligation(
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

pub(in crate::verification) fn exact_integer_shift_left_chain_cast_obligation(
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

pub(in crate::verification) fn exact_integer_shifted_interval_obligation(
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
