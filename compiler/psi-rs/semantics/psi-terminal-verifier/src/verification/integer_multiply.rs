//! Multiply and signed-product reduction strategies.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::affine_joins::{
    exact_integer_distinct_root_affine_product_join_obligation,
    exact_integer_same_root_affine_product_join_obligation,
};
use super::integer_divide_remainder::{is_maximum_divide, is_minimum_divide};
use super::{
    ExactIntegerAffineOperation, ExactIntegerIntervalPreimage, IntegerOffset,
    canonical_conjunction, checked_integer_ceil_division, checked_integer_floor_division,
    checked_signed_integer_product, exact_integer_affine_cast_affine_obligation,
    exact_integer_affine_chain_obligation, exact_integer_cast_chain_root_interval,
    exact_integer_cast_chain_then_affine_suffix_obligation,
    exact_integer_cast_then_affine_chain_obligation,
    exact_integer_computed_prefix_conversion_interval_obligation,
    exact_integer_divide_remainder_cast_affine_obligation,
    exact_integer_divide_remainder_then_affine_obligation,
    exact_integer_shift_cast_affine_obligation,
    exact_integer_shift_then_arithmetic_chain_obligation,
    exact_integer_signed_affine_cast_affine_obligation,
    exact_integer_signed_affine_chain_obligation, exact_integer_signed_affine_initial_form,
    exact_integer_signed_affine_preimage_interval, exact_integer_signed_affine_replay,
    exact_integer_signed_product_interval_obligation, exact_integer_source_interval_obligation,
    fixed_integer_type_interval, known_integer_term_value, landed_integer_constant_value,
    nonnegative_integer_factor, partial_fixed_native_integer_cast, signed_negative_magnitude,
};

pub(super) fn exact_integer_multiply_obligation_with_definitions(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_mul(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    if known_left.is_none()
        && known_right.is_none()
        && let Some(obligation) = exact_integer_same_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && known_right.is_none()
        && let Some(obligation) = exact_integer_distinct_root_affine_product_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    let (variable, constant, constant_term, chain_orientation) = match (known_left, known_right) {
        (Some(constant), None) => (right, constant, left, false),
        (None, Some(constant)) => (left, constant, right, true),
        (None, None) => {
            if integer_type.sign() == IntegerSign::Unsigned {
                let one = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(1))
                    .expect("one belongs to every unsigned carrier");
                for (variable, factor) in [(&left, &right), (&right, &left)] {
                    let positive = Proposition::LessOrEqual(one.clone(), factor.clone());
                    if !semantic_axioms.contains(&positive) {
                        continue;
                    }
                    if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_left, bound_right) => {
                            bound_left == variable
                                && is_maximum_divide(
                                    integer_type,
                                    bound_right,
                                    factor,
                                    semantic_axioms,
                                )
                        }
                        _ => false,
                    }) {
                        return canonical_conjunction(vec![positive, bound.clone()]);
                    }
                }
            }
            if integer_type.sign() == IntegerSign::Signed {
                let one = ScalarTerm::integer(integer_type, IntegerValue::Signed(1))
                    .expect("one belongs to every signed carrier");
                let negative_two = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2))
                    .expect("negative two belongs to every signed carrier");
                for (variable, factor) in [(&left, &right), (&right, &left)] {
                    let positive = Proposition::LessOrEqual(one.clone(), factor.clone());
                    if semantic_axioms.contains(&positive) {
                        let lower = semantic_axioms.iter().rev().find(|axiom| match axiom {
                            Proposition::LessOrEqual(bound, bound_variable) => {
                                bound_variable == variable
                                    && is_minimum_divide(
                                        integer_type,
                                        bound,
                                        factor,
                                        semantic_axioms,
                                    )
                            }
                            _ => false,
                        });
                        let upper = semantic_axioms.iter().rev().find(|axiom| match axiom {
                            Proposition::LessOrEqual(bound_variable, bound) => {
                                bound_variable == variable
                                    && is_maximum_divide(
                                        integer_type,
                                        bound,
                                        factor,
                                        semantic_axioms,
                                    )
                            }
                            _ => false,
                        });
                        if let (Some(lower), Some(upper)) = (lower, upper) {
                            return canonical_conjunction(vec![
                                positive,
                                lower.clone(),
                                upper.clone(),
                            ]);
                        }
                    }

                    let negative = Proposition::LessOrEqual(factor.clone(), negative_two.clone());
                    if !semantic_axioms.contains(&negative) {
                        continue;
                    }
                    let lower = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound, bound_variable) => {
                            bound_variable == variable
                                && is_maximum_divide(integer_type, bound, factor, semantic_axioms)
                        }
                        _ => false,
                    });
                    let upper = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_variable, bound) => {
                            bound_variable == variable
                                && is_minimum_divide(integer_type, bound, factor, semantic_axioms)
                        }
                        _ => false,
                    });
                    if let (Some(lower), Some(upper)) = (lower, upper) {
                        return canonical_conjunction(vec![negative, lower.clone(), upper.clone()]);
                    }
                }
            }
            return Proposition::Falsehood;
        }
        (Some(_), Some(_)) => unreachable!("known exact-multiply operands returned above"),
    };
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_then_arithmetic_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_signed_product_suffix_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_cast_affine_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_affine_suffix_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            ExactIntegerAffineOperation::Multiply,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if chain_orientation
        && landed_integer_constant_value(
            integer_type,
            &constant_term,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_multiply_chain_obligation(
            integer_type,
            variable.clone(),
            constant,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    match (integer_type.sign(), constant) {
        (IntegerSign::Unsigned, IntegerValue::Unsigned(0))
        | (IntegerSign::Unsigned, IntegerValue::Unsigned(1))
        | (IntegerSign::Signed, IntegerValue::Signed(0))
        | (IntegerSign::Signed, IntegerValue::Signed(1)) => Proposition::Truth,
        (IntegerSign::Unsigned, IntegerValue::Unsigned(constant)) => {
            let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
                unreachable!("unsigned type has unsigned maximum")
            };
            let boundary =
                ScalarTerm::integer(integer_type, IntegerValue::Unsigned(maximum / constant))
                    .expect("exact-multiply unsigned upper boundary remains in the carrier");
            Proposition::LessOrEqual(variable, boundary)
        }
        (IntegerSign::Signed, IntegerValue::Signed(-1)) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(
                    minimum
                        .checked_add(1)
                        .expect("fixed signed minimum is below maximum"),
                ),
            )
            .expect("exact-multiply negation boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, variable)
        }
        (IntegerSign::Signed, IntegerValue::Signed(constant)) if constant > 1 => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                unreachable!("signed type has signed bounds")
            };
            let lower = ScalarTerm::integer(integer_type, IntegerValue::Signed(minimum / constant))
                .expect("exact-multiply signed lower boundary remains in the carrier");
            let upper = ScalarTerm::integer(integer_type, IntegerValue::Signed(maximum / constant))
                .expect("exact-multiply signed upper boundary remains in the carrier");
            canonical_conjunction(vec![
                Proposition::LessOrEqual(lower, variable.clone()),
                Proposition::LessOrEqual(variable, upper),
            ])
        }
        (IntegerSign::Signed, IntegerValue::Signed(constant)) => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                unreachable!("signed type has signed bounds")
            };
            let lower = ScalarTerm::integer(integer_type, IntegerValue::Signed(maximum / constant))
                .expect("exact-multiply negative signed lower boundary remains in the carrier");
            let upper = ScalarTerm::integer(integer_type, IntegerValue::Signed(minimum / constant))
                .expect("exact-multiply negative signed upper boundary remains in the carrier");
            canonical_conjunction(vec![
                Proposition::LessOrEqual(lower, variable.clone()),
                Proposition::LessOrEqual(variable, upper),
            ])
        }
        _ => Proposition::Falsehood,
    }
}

fn exact_integer_cast_then_multiply_chain_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.is_address() || !matches!(target_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut cumulative_factor = nonnegative_integer_factor(target_type, factor)?;
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
        match definition {
            ScalarTerm::ExactIntegerMultiply {
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
                let nested_factor = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )
                .and_then(|factor| nonnegative_integer_factor(target_type, factor))?;
                let Some(product) = cumulative_factor.checked_mul(nested_factor) else {
                    return Some(Proposition::Falsehood);
                };
                cumulative_factor = product;
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
                && source_type.can_exact_cast_to(target_type)
                && matches!(
                    operand.as_ref(),
                    ScalarTerm::Value {
                        id,
                        scalar_type: ScalarType::Integer(root_type),
                    } if *root_type == *source_type && machine_parameter_values.contains(id)
                ) =>
            {
                return Some(exact_integer_scaled_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    cumulative_factor,
                ));
            }
            _ => return None,
        }
    }
    None
}

pub(super) fn exact_integer_cast_then_signed_affine_chain_obligation(
    target_type: psi_core::IntegerType,
    variable: ScalarTerm,
    initial_constant: IntegerValue,
    initial_operation: ExactIntegerAffineOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let (coefficient, offset, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_initial_form(initial_constant, initial_operation)?;
    let (variable, coefficient, offset, prior_axiom_count, saw_offset, saw_negative_factor) =
        exact_integer_signed_affine_replay(
            target_type,
            variable,
            coefficient,
            offset,
            saw_offset,
            saw_negative_factor,
            semantic_axioms,
            definition_axiom_count,
        )?;
    if !saw_offset || !saw_negative_factor {
        return None;
    }
    let (_, definition) = semantic_axioms[..prior_axiom_count]
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, axiom)| match axiom {
            Proposition::Equal(left, right) if left == &variable => Some((index, right)),
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
    if *cast_target_type != target_type
        || !partial_fixed_native_integer_cast(*source_type, target_type)
        || !matches!(
            operand.as_ref(),
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == *source_type && machine_parameter_values.contains(id)
        )
    {
        return None;
    }
    let interval = match exact_integer_signed_affine_preimage_interval(
        coefficient,
        offset,
        fixed_integer_type_interval(target_type)?,
    )? {
        ExactIntegerIntervalPreimage::Interval(interval) => interval,
        ExactIntegerIntervalPreimage::Empty => return Some(Proposition::Falsehood),
    };
    Some(exact_integer_source_interval_obligation(
        *source_type,
        (**operand).clone(),
        interval.0,
        interval.1,
    ))
}

pub(super) fn exact_integer_cast_then_signed_multiply_chain_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.sign() != IntegerSign::Signed
        || target_type.is_address()
        || !matches!(target_type.bits(), 8 | 16 | 32 | 64)
    {
        return None;
    }
    let IntegerValue::Signed(factor_value) = factor else {
        return None;
    };
    let mut product = checked_signed_integer_product(Some(IntegerOffset::Nonnegative(1)), factor);
    let mut saw_negative = factor_value < 0;
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
        match definition {
            ScalarTerm::ExactIntegerMultiply {
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
                let nested_factor = landed_integer_constant_value(
                    target_type,
                    right,
                    semantic_axioms,
                    definition_index,
                )?;
                let IntegerValue::Signed(nested_value) = nested_factor else {
                    return None;
                };
                product = checked_signed_integer_product(product, nested_factor);
                saw_negative |= nested_value < 0;
                variable = (**left).clone();
                prior_axiom_count = definition_index;
            }
            ScalarTerm::IntegerExactCast {
                source_type,
                target_type: cast_target_type,
                operand,
            } if saw_negative
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
                return exact_integer_signed_product_interval_obligation(
                    *source_type,
                    target_type,
                    (**operand).clone(),
                    product?,
                );
            }
            _ => return None,
        }
    }
    None
}

pub(super) fn exact_integer_cast_chain_then_signed_product_suffix_obligation(
    target_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if target_type.sign() != IntegerSign::Signed {
        return None;
    }
    let IntegerValue::Signed(factor_value) = factor else {
        return None;
    };
    let mut product = checked_signed_integer_product(Some(IntegerOffset::Nonnegative(1)), factor);
    let mut saw_negative = factor_value < 0;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    for _ in 0..=prior_axiom_count {
        if saw_negative
            && let Some((root_type, root, cast_interval)) = exact_integer_cast_chain_root_interval(
                target_type,
                variable.clone(),
                semantic_axioms,
                prior_axiom_count,
                machine_parameter_values,
            )
        {
            let product = product?;
            if product.magnitude() == 0 {
                return Some(Proposition::Truth);
            }
            let target_interval = fixed_integer_type_interval(target_type)?;
            let magnitude = product.magnitude();
            let preimage = if magnitude > i128::MAX as u128 {
                (0, 0)
            } else {
                let magnitude = i128::try_from(magnitude).ok()?;
                let signed_product = match product {
                    IntegerOffset::Nonnegative(_) => magnitude,
                    IntegerOffset::Negative(_) => magnitude.checked_neg()?,
                };
                if signed_product > 0 {
                    (
                        checked_integer_ceil_division(target_interval.0, signed_product)?,
                        checked_integer_floor_division(target_interval.1, signed_product)?,
                    )
                } else {
                    (
                        checked_integer_ceil_division(target_interval.1, signed_product)?,
                        checked_integer_floor_division(target_interval.0, signed_product)?,
                    )
                }
            };
            let minimum = preimage.0.max(cast_interval.0);
            let maximum = preimage.1.min(cast_interval.1);
            return Some(if minimum <= maximum {
                exact_integer_source_interval_obligation(root_type, root, minimum, maximum)
            } else {
                Proposition::Falsehood
            });
        }
        if saw_negative {
            let product = product?;
            let target_interval = fixed_integer_type_interval(target_type)?;
            if product.magnitude() == 0 {
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
                    return Some(Proposition::Truth);
                }
            } else {
                let magnitude = product.magnitude();
                let preimage = if magnitude > i128::MAX as u128 {
                    (0, 0)
                } else {
                    let magnitude = i128::try_from(magnitude).ok()?;
                    let signed_product = match product {
                        IntegerOffset::Nonnegative(_) => magnitude,
                        IntegerOffset::Negative(_) => magnitude.checked_neg()?,
                    };
                    if signed_product > 0 {
                        (
                            checked_integer_ceil_division(target_interval.0, signed_product)?,
                            checked_integer_floor_division(target_interval.1, signed_product)?,
                        )
                    } else {
                        (
                            checked_integer_ceil_division(target_interval.1, signed_product)?,
                            checked_integer_floor_division(target_interval.0, signed_product)?,
                        )
                    }
                };
                if let Some(obligation) =
                    exact_integer_computed_prefix_conversion_interval_obligation(
                        target_type,
                        variable.clone(),
                        preimage,
                        semantic_axioms,
                        prior_axiom_count,
                        machine_parameter_values,
                    )
                {
                    return Some(obligation);
                }
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
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            return None;
        };
        if *scalar_type != target_type
            || landed_integer_constant_value(target_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            return None;
        }
        let nested_factor =
            landed_integer_constant_value(target_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(nested_value) = nested_factor else {
            return None;
        };
        product = checked_signed_integer_product(product, nested_factor);
        saw_negative |= nested_value < 0;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
    }
    None
}

fn exact_integer_scaled_interval_obligation(
    root_type: psi_core::IntegerType,
    interval_type: psi_core::IntegerType,
    root: ScalarTerm,
    cumulative_factor: u128,
) -> Proposition {
    if cumulative_factor <= 1 {
        return Proposition::Truth;
    }
    let (target_minimum, target_maximum) = match interval_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = interval_type.maximum_value() else {
                unreachable!("unsigned fixed integer type has an unsigned maximum")
            };
            let Some(maximum) = i128::try_from(maximum / cumulative_factor).ok() else {
                return Proposition::Falsehood;
            };
            (0, maximum)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (interval_type.minimum_value(), interval_type.maximum_value())
            else {
                unreachable!("signed fixed integer type has signed bounds")
            };
            let Some(minimum) =
                signed_negative_magnitude(minimum.unsigned_abs() / cumulative_factor)
            else {
                return Proposition::Falsehood;
            };
            let Some(maximum) = i128::try_from(maximum as u128 / cumulative_factor).ok() else {
                return Proposition::Falsehood;
            };
            (minimum, maximum)
        }
    };
    exact_integer_source_interval_obligation(root_type, root, target_minimum, target_maximum)
}

#[cfg(test)]
pub(super) fn exact_integer_multiply_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    exact_integer_multiply_obligation_with_definitions(
        integer_type,
        left,
        right,
        semantic_axioms,
        0,
        &BTreeSet::new(),
    )
}

fn exact_integer_multiply_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    let mut cumulative_factor = nonnegative_integer_factor(integer_type, factor)?;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
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
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            break;
        };
        if *scalar_type != integer_type
            || landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            break;
        }
        let Some(nested_factor) =
            landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
                .and_then(|factor| nonnegative_integer_factor(integer_type, factor))
        else {
            break;
        };
        let Some(product) = cumulative_factor.checked_mul(nested_factor) else {
            return Some(Proposition::Falsehood);
        };
        cumulative_factor = product;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
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
    Some(exact_integer_cumulative_multiply_obligation(
        integer_type,
        variable,
        cumulative_factor,
    ))
}

pub(super) fn exact_integer_signed_multiply_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    factor: IntegerValue,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.sign() != IntegerSign::Signed {
        return None;
    }
    let IntegerValue::Signed(factor_value) = factor else {
        return None;
    };
    let mut product = checked_signed_integer_product(Some(IntegerOffset::Nonnegative(1)), factor);
    let mut saw_negative = factor_value < 0;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let mut followed_definition = false;
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
        let ScalarTerm::ExactIntegerMultiply {
            scalar_type,
            left,
            right,
        } = definition
        else {
            break;
        };
        if *scalar_type != integer_type
            || landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
                .is_some()
        {
            break;
        }
        let nested_factor =
            landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)?;
        let IntegerValue::Signed(nested_value) = nested_factor else {
            return None;
        };
        product = checked_signed_integer_product(product, nested_factor);
        saw_negative |= nested_value < 0;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
    }
    if !followed_definition
        || !saw_negative
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
    exact_integer_signed_product_interval_obligation(integer_type, integer_type, variable, product?)
}

fn exact_integer_cumulative_multiply_obligation(
    integer_type: psi_core::IntegerType,
    variable: ScalarTerm,
    cumulative_factor: u128,
) -> Proposition {
    if cumulative_factor <= 1 {
        return Proposition::Truth;
    }
    match integer_type.sign() {
        IntegerSign::Unsigned => {
            let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
                unreachable!("unsigned type has unsigned maximum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Unsigned(maximum / cumulative_factor),
            )
            .expect("cumulative exact-multiply upper boundary remains in the carrier");
            Proposition::LessOrEqual(variable, boundary)
        }
        IntegerSign::Signed => {
            let (IntegerValue::Signed(minimum), IntegerValue::Signed(maximum)) =
                (integer_type.minimum_value(), integer_type.maximum_value())
            else {
                unreachable!("signed type has signed bounds")
            };
            let lower = signed_negative_magnitude(minimum.unsigned_abs() / cumulative_factor)
                .expect("cumulative exact-multiply lower boundary remains signed");
            let upper = i128::try_from(maximum as u128 / cumulative_factor)
                .expect("cumulative exact-multiply upper boundary remains signed");
            canonical_conjunction(vec![
                Proposition::LessOrEqual(
                    ScalarTerm::integer(integer_type, IntegerValue::Signed(lower))
                        .expect("cumulative exact-multiply lower boundary remains in the carrier"),
                    variable.clone(),
                ),
                Proposition::LessOrEqual(
                    variable,
                    ScalarTerm::integer(integer_type, IntegerValue::Signed(upper))
                        .expect("cumulative exact-multiply upper boundary remains in the carrier"),
                ),
            ])
        }
    }
}
