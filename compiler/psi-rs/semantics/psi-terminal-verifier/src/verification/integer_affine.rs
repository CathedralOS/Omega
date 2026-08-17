//! Shared affine inverse and composition engine.
//!
//! Arithmetic strategy modules use this engine to derive sufficient affine
//! preimages during the canonical-semantic-ledger migration. It does not own
//! artifact traversal, canonical goal identity, or proof acceptance.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::{
    ExactIntegerAffineOperation, ExactIntegerIntervalPreimage, IntegerOffset,
    exact_integer_affine_preimage_interval, exact_integer_affine_preimage_obligation,
    exact_integer_cast_chain_root_interval,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_source_interval_obligation, fixed_integer_type_interval,
    landed_integer_constant_value, nonnegative_integer_factor, partial_fixed_native_integer_cast,
    signed_negative_magnitude,
};

pub(super) fn exact_integer_signed_affine_initial_form(
    constant: IntegerValue,
    operation: ExactIntegerAffineOperation,
) -> Option<(IntegerOffset, IntegerOffset, bool, bool)> {
    let IntegerValue::Signed(constant_value) = constant else {
        return None;
    };
    let constant = IntegerOffset::from_value(constant);
    Some(match operation {
        ExactIntegerAffineOperation::Add => (IntegerOffset::Nonnegative(1), constant, true, false),
        ExactIntegerAffineOperation::Subtract => (
            IntegerOffset::Nonnegative(1),
            constant.negated(),
            true,
            false,
        ),
        ExactIntegerAffineOperation::Multiply => (
            constant,
            IntegerOffset::Nonnegative(0),
            false,
            constant_value < 0,
        ),
    })
}

pub(super) fn exact_integer_signed_affine_replay(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    mut coefficient: IntegerOffset,
    mut offset: IntegerOffset,
    mut saw_offset: bool,
    mut saw_negative_factor: bool,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
) -> Option<(ScalarTerm, IntegerOffset, IntegerOffset, usize, bool, bool)> {
    if integer_type.sign() != IntegerSign::Signed
        || integer_type.is_address()
        || !matches!(integer_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::Nonnegative(1),
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                IntegerOffset::Nonnegative(1),
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => {
                let factor = landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let IntegerValue::Signed(_) = factor else {
                    return None;
                };
                (
                    left,
                    right,
                    IntegerOffset::from_value(factor),
                    IntegerOffset::Nonnegative(0),
                    ExactIntegerAffineOperation::Multiply,
                )
            }
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply_offset(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_multiply_offset(nested_coefficient)?;
        if operation == ExactIntegerAffineOperation::Multiply {
            saw_negative_factor |= matches!(nested_coefficient, IntegerOffset::Negative(_));
        } else {
            saw_offset = true;
        }
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    Some((
        variable,
        coefficient,
        offset,
        prior_axiom_count,
        saw_offset,
        saw_negative_factor,
    ))
}

pub(super) fn exact_integer_signed_affine_preimage_interval(
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<ExactIntegerIntervalPreimage> {
    if interval.0 > interval.1 {
        return Some(ExactIntegerIntervalPreimage::Empty);
    }
    if coefficient.magnitude() == 0 {
        let constant = match offset {
            IntegerOffset::Nonnegative(value) => i128::try_from(value).ok(),
            IntegerOffset::Negative(value) => signed_negative_magnitude(value),
        };
        return Some(
            if constant.is_some_and(|value| interval.0 <= value && value <= interval.1) {
                ExactIntegerIntervalPreimage::Interval((i128::MIN, i128::MAX))
            } else {
                ExactIntegerIntervalPreimage::Empty
            },
        );
    }
    let minimum = IntegerOffset::from_value(IntegerValue::Signed(interval.0));
    let maximum = IntegerOffset::from_value(IntegerValue::Signed(interval.1));
    let (lower_numerator, upper_numerator) = match coefficient {
        IntegerOffset::Nonnegative(_) => (
            minimum.checked_add(offset.negated())?,
            maximum.checked_add(offset.negated())?,
        ),
        IntegerOffset::Negative(_) => (
            offset.checked_add(maximum.negated())?,
            offset.checked_add(minimum.negated())?,
        ),
    };
    let lower = integer_offset_ceil_div(lower_numerator, coefficient.magnitude());
    let upper = integer_offset_floor_div(upper_numerator, coefficient.magnitude());
    let lower = match lower {
        IntegerOffset::Nonnegative(value) => match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => return Some(ExactIntegerIntervalPreimage::Empty),
        },
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => i128::MIN,
        IntegerOffset::Negative(value) => signed_negative_magnitude(value)?,
    };
    let upper = match upper {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).unwrap_or(i128::MAX),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => {
            return Some(ExactIntegerIntervalPreimage::Empty);
        }
        IntegerOffset::Negative(value) => signed_negative_magnitude(value)?,
    };
    Some(if lower > upper {
        ExactIntegerIntervalPreimage::Empty
    } else {
        ExactIntegerIntervalPreimage::Interval((lower, upper))
    })
}

pub(super) fn exact_integer_signed_affine_interval_obligation(
    root_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: IntegerOffset,
    offset: IntegerOffset,
    interval: (i128, i128),
) -> Option<Proposition> {
    Some(
        match exact_integer_signed_affine_preimage_interval(coefficient, offset, interval)? {
            ExactIntegerIntervalPreimage::Interval((minimum, maximum)) => {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            }
            ExactIntegerIntervalPreimage::Empty => Proposition::Falsehood,
        },
    )
}

pub(super) fn exact_integer_signed_affine_chain_obligation(
    integer_type: psi_core::IntegerType,
    variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (coefficient, offset, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_initial_form(initial_constant, initial_operation)?;
    let (variable, coefficient, offset, _, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_replay(
            integer_type,
            variable,
            coefficient,
            offset,
            saw_offset,
            saw_negative_factor,
            semantic_axioms,
            definition_axiom_count,
        )?;
    if !saw_offset
        || !saw_negative_factor
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    exact_integer_signed_affine_interval_obligation(
        integer_type,
        variable,
        coefficient,
        offset,
        fixed_integer_type_interval(integer_type)?,
    )
}

pub(super) fn exact_integer_affine_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut coefficient, mut offset) = match initial_operation {
        ExactIntegerAffineOperation::Add => (1, IntegerOffset::from_value(initial_constant)),
        ExactIntegerAffineOperation::Subtract => {
            (1, IntegerOffset::from_subtrahend(initial_constant))
        }
        ExactIntegerAffineOperation::Multiply => (
            nonnegative_integer_factor(integer_type, initial_constant)?,
            IntegerOffset::Nonnegative(0),
        ),
    };
    let mut saw_offset = initial_operation != ExactIntegerAffineOperation::Multiply;
    let mut saw_multiply = initial_operation == ExactIntegerAffineOperation::Multiply;
    let mut followed_definition = false;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..prior_axiom_count {
        let Some((definition_index, definition)) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })
        else {
            break;
        };
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    integer_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (
                left,
                right,
                nonnegative_integer_factor(
                    integer_type,
                    landed_integer_constant_value(
                        integer_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
                ExactIntegerAffineOperation::Multiply,
            ),
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            break;
        }
        let Some(composed_offset) = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))
        else {
            return Some(Proposition::Falsehood);
        };
        let Some(composed_coefficient) = coefficient.checked_mul(nested_coefficient) else {
            return Some(Proposition::Falsehood);
        };
        coefficient = composed_coefficient;
        offset = composed_offset;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
        saw_offset |= operation != ExactIntegerAffineOperation::Multiply;
        saw_multiply |= operation == ExactIntegerAffineOperation::Multiply;
    }
    if !followed_definition
        || !saw_offset
        || !saw_multiply
        || !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    Some(exact_integer_affine_interval_obligation(
        integer_type,
        variable,
        coefficient,
        offset,
    ))
}

pub(super) fn exact_integer_affine_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let (mut target_coefficient, mut target_offset) = match initial_operation {
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
    let (source_type, mut source_variable) = loop {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
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
                target_offset = nested_offset
                    .checked_multiply(target_coefficient)
                    .and_then(|nested| nested.checked_add(target_offset))?;
                target_coefficient = target_coefficient.checked_mul(nested_coefficient)?;
                variable = (**left).clone();
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
    let target_preimage = if target_coefficient == 0 {
        None
    } else {
        match exact_integer_affine_preimage_interval(
            target_type,
            target_coefficient,
            target_offset,
            target_carrier,
        ) {
            Ok(interval) => interval,
            Err(()) => return None,
        }
    };
    let mut source_coefficient = 1_u128;
    let mut source_offset = IntegerOffset::Nonnegative(0);
    let mut followed_source_definition = false;
    for _ in 0..=prior_axiom_count {
        if matches!(
            &source_variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == source_type && machine_parameter_values.contains(id)
        ) {
            if !followed_source_definition {
                return None;
            }
            if target_coefficient == 0 {
                return Some(if target_offset.is_representable(target_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            let Some((target_lower, target_upper)) = target_preimage else {
                return Some(Proposition::Falsehood);
            };
            let source_carrier = fixed_integer_type_interval(source_type)?;
            let lower = target_lower.max(source_carrier.0);
            let upper = target_upper.min(source_carrier.1);
            if lower > upper {
                return Some(Proposition::Falsehood);
            }
            return exact_integer_affine_preimage_obligation(
                source_type,
                source_variable,
                source_coefficient,
                source_offset,
                (lower, upper),
            )
            .ok();
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &source_variable => Some((index, right)),
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
        source_offset = nested_offset
            .checked_multiply(source_coefficient)
            .and_then(|nested| nested.checked_add(source_offset))?;
        source_coefficient = source_coefficient.checked_mul(nested_coefficient)?;
        source_variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_source_definition = true;
    }
    None
}

pub(super) fn exact_integer_signed_affine_cast_affine_obligation(
    target_type: psi_core::IntegerType,
    variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (target_coefficient, target_offset, target_saw_offset, target_saw_negative_factor) =
        exact_integer_signed_affine_initial_form(initial_constant, initial_operation)?;
    let (
        cast_value,
        target_coefficient,
        target_offset,
        cast_definition_count,
        target_saw_offset,
        target_saw_negative_factor,
    ) = exact_integer_signed_affine_replay(
        target_type,
        variable,
        target_coefficient,
        target_offset,
        target_saw_offset,
        target_saw_negative_factor,
        semantic_axioms,
        definition_axiom_count,
    )?;
    let (cast_definition_index, cast_definition) = semantic_axioms[..cast_definition_count]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, axiom)| match axiom {
            Proposition::Equal(left, right) if left == &cast_value => Some((index, right)),
            _ => None,
        })?;
    let ScalarTerm::IntegerExactCast {
        source_type,
        target_type: cast_target_type,
        operand,
    } = cast_definition
    else {
        return None;
    };
    if *cast_target_type != target_type
        || source_type.sign() != IntegerSign::Signed
        || !partial_fixed_native_integer_cast(*source_type, target_type)
    {
        return None;
    }
    let (
        source_root,
        source_coefficient,
        source_offset,
        source_definition_count,
        source_saw_offset,
        source_saw_negative_factor,
    ) = exact_integer_signed_affine_replay(
        *source_type,
        (**operand).clone(),
        IntegerOffset::Nonnegative(1),
        IntegerOffset::Nonnegative(0),
        false,
        false,
        semantic_axioms,
        cast_definition_index,
    )?;
    let source_followed_definition = source_definition_count < cast_definition_index;
    if !source_followed_definition
        || !matches!(
            &source_root,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *source_type && machine_parameter_values.contains(id)
        )
        || !((source_saw_offset && source_saw_negative_factor)
            || (!source_saw_negative_factor && target_saw_offset && target_saw_negative_factor))
    {
        return None;
    }
    if target_coefficient.magnitude() == 0 {
        return Some(if target_offset.is_representable(target_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        });
    }
    let target_preimage = match exact_integer_signed_affine_preimage_interval(
        target_coefficient,
        target_offset,
        fixed_integer_type_interval(target_type)?,
    )? {
        ExactIntegerIntervalPreimage::Interval(interval) => interval,
        ExactIntegerIntervalPreimage::Empty => return Some(Proposition::Falsehood),
    };
    let source_carrier = fixed_integer_type_interval(*source_type)?;
    let target_carrier = fixed_integer_type_interval(target_type)?;
    let lower = target_preimage
        .0
        .max(source_carrier.0)
        .max(target_carrier.0);
    let upper = target_preimage
        .1
        .min(source_carrier.1)
        .min(target_carrier.1);
    if lower > upper {
        return Some(Proposition::Falsehood);
    }
    exact_integer_signed_affine_interval_obligation(
        *source_type,
        source_root,
        source_coefficient,
        source_offset,
        (lower, upper),
    )
}

pub(super) fn exact_integer_cast_then_affine_chain_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
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
    let mut saw_offset = initial_operation != ExactIntegerAffineOperation::Multiply;
    let mut saw_multiply = initial_operation == ExactIntegerAffineOperation::Multiply;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Add,
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
                ExactIntegerAffineOperation::Subtract,
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                nonnegative_integer_factor(
                    target_type,
                    landed_integer_constant_value(
                        target_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
                ExactIntegerAffineOperation::Multiply,
            ),
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if saw_offset
                && saw_multiply
                && *cast_target_type == target_type
                && !source_type.is_address()
                && matches!(source_type.bits(), 8 | 16 | 32 | 64)
                && *source_type != target_type
                && !source_type.can_widen_to(target_type)
                && source_type.can_exact_cast_to(target_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                return Some(exact_integer_affine_target_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    coefficient,
                    offset,
                ));
            }
            _ => return None,
        };
        if landed_integer_constant_value(target_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(target_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        let Some(composed_offset) = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))
        else {
            return Some(Proposition::Falsehood);
        };
        let Some(composed_coefficient) = coefficient.checked_mul(nested_coefficient) else {
            return Some(Proposition::Falsehood);
        };
        coefficient = composed_coefficient;
        offset = composed_offset;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        saw_offset |= operation != ExactIntegerAffineOperation::Multiply;
        saw_multiply |= operation == ExactIntegerAffineOperation::Multiply;
    }
    None
}

pub(super) fn exact_integer_cast_chain_then_affine_suffix_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
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
    for _ in 0..=prior_axiom_count {
        if let Some((root_type, root, cast_interval)) = exact_integer_cast_chain_root_interval(
            target_type,
            variable.clone(),
            semantic_axioms,
            prior_axiom_count,
            machine_parameter_values,
        ) {
            if coefficient == 0 {
                return Some(if offset.is_representable(target_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
            let target_interval = fixed_integer_type_interval(target_type)?;
            let preimage = match exact_integer_affine_preimage_interval(
                target_type,
                coefficient,
                offset,
                target_interval,
            ) {
                Ok(Some(interval)) => interval,
                Ok(None) => return Some(Proposition::Falsehood),
                Err(()) => return None,
            };
            let minimum = preimage.0.max(cast_interval.0);
            let maximum = preimage.1.min(cast_interval.1);
            return Some(if minimum <= maximum {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            } else {
                Proposition::Falsehood
            });
        }
        let target_interval = fixed_integer_type_interval(target_type)?;
        if coefficient == 0 {
            if exact_integer_computed_prefix_conversion_interval_obligation(
                target_type,
                variable.clone(),
                target_interval,
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
            .is_some()
            {
                return Some(if offset.is_representable(target_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                });
            }
        } else {
            match exact_integer_affine_preimage_interval(
                target_type,
                coefficient,
                offset,
                target_interval,
            ) {
                Ok(Some(interval)) => {
                    if let Some(obligation) =
                        exact_integer_computed_prefix_conversion_interval_obligation(
                            target_type,
                            variable.clone(),
                            interval,
                            semantic_axioms,
                            prior_axiom_count,
                            machine_parameter_values,
                        )
                    {
                        return Some(obligation);
                    }
                }
                Ok(None) => {
                    if exact_integer_computed_prefix_conversion_interval_obligation(
                        target_type,
                        variable.clone(),
                        target_interval,
                        semantic_axioms,
                        prior_axiom_count,
                        machine_parameter_values,
                    )
                    .is_some()
                    {
                        return Some(Proposition::Falsehood);
                    }
                }
                Err(()) => return None,
            }
        }
        let (definition_index, definition) = semantic_axioms[..prior_axiom_count]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, axiom)| match axiom {
                Proposition::Equal(left, right) if left == &variable => Some((index, right)),
                _ => None,
            })?;
        let (left, right, nested_coefficient, nested_offset) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_value(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                1,
                IntegerOffset::from_subtrahend(landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?),
            ),
            ScalarTerm::ExactIntegerMultiply {
                scalar_type,
                left,
                right,
            } if *scalar_type == target_type => (
                left,
                right,
                nonnegative_integer_factor(
                    target_type,
                    landed_integer_constant_value(
                        target_type,
                        right,
                        semantic_axioms,
                        definition_index,
                    )?,
                )?,
                IntegerOffset::Nonnegative(0),
            ),
            _ => return None,
        };
        if landed_integer_constant_value(target_type, left, semantic_axioms, definition_index)
            .is_some()
            || landed_integer_constant_value(target_type, right, semantic_axioms, definition_index)
                .is_none()
        {
            return None;
        }
        offset = nested_offset
            .checked_multiply(coefficient)
            .and_then(|nested| nested.checked_add(offset))?;
        coefficient = coefficient.checked_mul(nested_coefficient)?;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    None
}

pub(super) fn exact_integer_affine_interval_obligation(
    integer_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: u128,
    offset: IntegerOffset,
) -> Proposition {
    exact_integer_affine_target_interval_obligation(
        integer_type,
        integer_type,
        root,
        coefficient,
        offset,
    )
}

pub(super) fn exact_integer_affine_target_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    coefficient: u128,
    offset: IntegerOffset,
) -> Proposition {
    if coefficient == 0 {
        return if offset.is_representable(interval_type) {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    let minimum = IntegerOffset::from_value(interval_type.minimum_value());
    let maximum = IntegerOffset::from_value(interval_type.maximum_value());
    let Some(lower_numerator) = minimum.checked_add(offset.negated()) else {
        return Proposition::Falsehood;
    };
    let Some(upper_numerator) = maximum.checked_add(offset.negated()) else {
        return Proposition::Falsehood;
    };
    let lower = integer_offset_ceil_div(lower_numerator, coefficient);
    let upper = integer_offset_floor_div(upper_numerator, coefficient);
    let lower = match lower {
        IntegerOffset::Nonnegative(value) => match i128::try_from(value) {
            Ok(value) => value,
            Err(_) => return Proposition::Falsehood,
        },
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => i128::MIN,
        IntegerOffset::Negative(value) => match signed_negative_magnitude(value) {
            Some(value) => value,
            None => return Proposition::Falsehood,
        },
    };
    let upper = match upper {
        IntegerOffset::Nonnegative(value) => i128::try_from(value).unwrap_or(i128::MAX),
        IntegerOffset::Negative(value) if value > (1_u128 << 127) => {
            return Proposition::Falsehood;
        }
        IntegerOffset::Negative(value) => match signed_negative_magnitude(value) {
            Some(value) => value,
            None => return Proposition::Falsehood,
        },
    };
    exact_integer_source_interval_obligation(root_type, root, lower, upper)
}

pub(super) fn integer_offset_floor_div(value: IntegerOffset, divisor: u128) -> IntegerOffset {
    debug_assert_ne!(divisor, 0);
    match value {
        IntegerOffset::Nonnegative(value) => IntegerOffset::Nonnegative(value / divisor),
        IntegerOffset::Negative(value) => {
            let quotient = value / divisor;
            let magnitude = quotient + u128::from(value % divisor != 0);
            if magnitude == 0 {
                IntegerOffset::Nonnegative(0)
            } else {
                IntegerOffset::Negative(magnitude)
            }
        }
    }
}

pub(super) fn integer_offset_ceil_div(value: IntegerOffset, divisor: u128) -> IntegerOffset {
    debug_assert_ne!(divisor, 0);
    match value {
        IntegerOffset::Nonnegative(value) => {
            let quotient = value / divisor;
            IntegerOffset::Nonnegative(quotient + u128::from(value % divisor != 0))
        }
        IntegerOffset::Negative(value) => {
            let magnitude = value / divisor;
            if magnitude == 0 {
                IntegerOffset::Nonnegative(0)
            } else {
                IntegerOffset::Negative(magnitude)
            }
        }
    }
}
