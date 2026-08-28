//! Add, subtract, and mixed offset-chain reduction strategies.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, ScalarTerm, ScalarType, ValueId};

use super::affine_joins::{
    exact_integer_affine_fork_join_obligation,
    exact_integer_distinct_root_affine_fork_join_obligation,
};
use super::integer_multiply::exact_integer_cast_then_signed_affine_chain_obligation;
use super::{
    ExactIntegerAffineOperation, ExactIntegerOffsetOperation, IntegerOffset, canonical_conjunction,
    exact_integer_affine_cast_affine_obligation, exact_integer_affine_chain_obligation,
    exact_integer_cast_chain_then_affine_suffix_obligation,
    exact_integer_cast_then_affine_chain_obligation, exact_integer_cast_then_offset_obligation,
    exact_integer_divide_remainder_cast_affine_obligation,
    exact_integer_divide_remainder_then_affine_obligation,
    exact_integer_shift_cast_affine_obligation,
    exact_integer_shift_then_arithmetic_chain_obligation,
    exact_integer_signed_affine_cast_affine_obligation,
    exact_integer_signed_affine_chain_obligation, integer_type_span, known_integer_term_value,
    landed_integer_constant_value, signed_negative_magnitude,
};

fn exact_integer_offset_obligation(
    integer_type: psi_core::IntegerType,
    variable: ScalarTerm,
    offset: IntegerOffset,
) -> Proposition {
    if offset.magnitude() > integer_type_span(integer_type) {
        return Proposition::Falsehood;
    }
    match (integer_type.sign(), offset) {
        (_, IntegerOffset::Nonnegative(0)) | (_, IntegerOffset::Negative(0)) => Proposition::Truth,
        (IntegerSign::Unsigned, IntegerOffset::Nonnegative(offset)) => {
            let IntegerValue::Unsigned(maximum) = integer_type.maximum_value() else {
                unreachable!("unsigned type has unsigned maximum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Unsigned(maximum.checked_sub(offset).expect("offset fits span")),
            )
            .expect("exact-add unsigned boundary remains in the carrier");
            Proposition::LessOrEqual(variable, boundary)
        }
        (IntegerSign::Unsigned, IntegerOffset::Negative(offset)) => {
            let boundary = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(offset))
                .expect("exact-subtract unsigned boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, variable)
        }
        (IntegerSign::Signed, IntegerOffset::Nonnegative(offset)) => {
            let IntegerValue::Signed(maximum) = integer_type.maximum_value() else {
                unreachable!("signed type has signed maximum")
            };
            let boundary = if offset <= maximum as u128 {
                maximum - offset as i128
            } else {
                signed_negative_magnitude(offset - maximum as u128)
                    .expect("offset within the carrier span has a signed boundary")
            };
            Proposition::LessOrEqual(
                variable,
                ScalarTerm::integer(integer_type, IntegerValue::Signed(boundary))
                    .expect("exact-add signed upper boundary remains in the carrier"),
            )
        }
        (IntegerSign::Signed, IntegerOffset::Negative(offset)) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let minimum_magnitude = minimum.unsigned_abs();
            let boundary = if offset < minimum_magnitude {
                signed_negative_magnitude(minimum_magnitude - offset)
                    .expect("offset within the carrier span has a signed boundary")
            } else {
                i128::try_from(offset - minimum_magnitude)
                    .expect("offset within the carrier span has a signed boundary")
            };
            Proposition::LessOrEqual(
                ScalarTerm::integer(integer_type, IntegerValue::Signed(boundary))
                    .expect("exact-add signed lower boundary remains in the carrier"),
                variable,
            )
        }
    }
}

pub(super) fn exact_integer_add_obligation(
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
        return if integer_type.exact_add(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    if known_left.is_none()
        && known_right.is_none()
        && let Some(obligation) = exact_integer_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && known_right.is_none()
        && let Some(obligation) = exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_then_arithmetic_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_affine_suffix_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_offset_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_value(constant),
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_mixed_add_subtract_chain_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_value(constant),
            ExactIntegerOffsetOperation::Add,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    let (mut variable, constant, constant_term) = match (known_left, known_right) {
        (Some(constant), None) => (right, constant, left),
        (None, Some(constant)) => (left, constant, right),
        (None, None) => {
            if integer_type.sign() == IntegerSign::Unsigned {
                if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                    Proposition::LessOrEqual(bound_left, bound_right) => {
                        (bound_left == &left
                            && is_maximum_minus(integer_type, bound_right, &right, semantic_axioms))
                            || (bound_left == &right
                                && is_maximum_minus(
                                    integer_type,
                                    bound_right,
                                    &left,
                                    semantic_axioms,
                                ))
                    }
                    _ => false,
                }) {
                    return bound.clone();
                }
            }
            if integer_type.sign() == IntegerSign::Signed {
                let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                    .expect("zero belongs to every signed carrier");
                for (variable, addend) in [(&left, &right), (&right, &left)] {
                    let nonnegative = Proposition::LessOrEqual(zero.clone(), addend.clone());
                    if !semantic_axioms.contains(&nonnegative) {
                        continue;
                    }
                    if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_left, bound_right) => {
                            bound_left == variable
                                && is_maximum_minus(
                                    integer_type,
                                    bound_right,
                                    addend,
                                    semantic_axioms,
                                )
                        }
                        _ => false,
                    }) {
                        return canonical_conjunction(vec![nonnegative, bound.clone()]);
                    }
                }
            }
            if integer_type.sign() == IntegerSign::Signed {
                let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                    .expect("zero belongs to every signed carrier");
                for (variable, addend) in [(&left, &right), (&right, &left)] {
                    let nonpositive = Proposition::LessOrEqual(addend.clone(), zero.clone());
                    if !semantic_axioms.contains(&nonpositive) {
                        continue;
                    }
                    if let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                        Proposition::LessOrEqual(bound_left, bound_right) => {
                            bound_right == variable
                                && is_minimum_minus(
                                    integer_type,
                                    bound_left,
                                    addend,
                                    semantic_axioms,
                                )
                        }
                        _ => false,
                    }) {
                        return canonical_conjunction(vec![nonpositive, bound.clone()]);
                    }
                }
            }
            return Proposition::Falsehood;
        }
        (Some(_), Some(_)) => unreachable!("known exact-add operands returned above"),
    };
    let original_variable = variable.clone();
    let original_offset = IntegerOffset::from_value(constant);
    let mut offset = original_offset;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let may_follow_chain = landed_integer_constant_value(
        integer_type,
        &constant_term,
        semantic_axioms,
        prior_axiom_count,
    ) == Some(constant);
    let mut followed_definition = false;
    if may_follow_chain {
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
            let ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } = definition
            else {
                break;
            };
            if *scalar_type != integer_type {
                break;
            }
            let known_left = landed_integer_constant_value(
                integer_type,
                left,
                semantic_axioms,
                definition_index,
            );
            let known_right = landed_integer_constant_value(
                integer_type,
                right,
                semantic_axioms,
                definition_index,
            );
            let (nested_variable, nested_constant) = match (known_left, known_right) {
                (Some(left), None) => ((**right).clone(), left),
                (None, Some(right)) => ((**left).clone(), right),
                (Some(left), Some(right)) => {
                    let Some(base) = integer_type.exact_add(left, right) else {
                        return Proposition::Falsehood;
                    };
                    let Some(total) = offset.checked_add(IntegerOffset::from_value(base)) else {
                        return Proposition::Falsehood;
                    };
                    return if total.is_representable(integer_type) {
                        Proposition::Truth
                    } else {
                        Proposition::Falsehood
                    };
                }
                (None, None) => break,
            };
            let Some(combined) = offset.checked_add(IntegerOffset::from_value(nested_constant))
            else {
                return Proposition::Falsehood;
            };
            if combined.magnitude() > integer_type_span(integer_type) {
                return Proposition::Falsehood;
            }
            offset = combined;
            variable = nested_variable;
            prior_axiom_count = definition_index;
            followed_definition = true;
        }
    }
    if followed_definition
        && !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        variable = original_variable;
        offset = original_offset;
    }
    exact_integer_offset_obligation(integer_type, variable, offset)
}

fn is_maximum_minus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    subtrahend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == subtrahend
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.maximum_value())
}

fn is_minimum_minus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    subtrahend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerSubtract {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == subtrahend
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.minimum_value())
}

pub(super) fn exact_integer_subtract_obligation(
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
        return if integer_type.exact_sub(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    let Some(constant) = known_right else {
        if let Some(obligation) = exact_integer_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if let Some(obligation) = exact_integer_distinct_root_affine_fork_join_obligation(
            integer_type,
            left.clone(),
            right.clone(),
            ExactIntegerOffsetOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        ) {
            return obligation;
        }
        if integer_type.sign() == IntegerSign::Unsigned {
            let bound = Proposition::LessOrEqual(right.clone(), left.clone());
            if semantic_axioms.contains(&bound) {
                return bound;
            }
        }
        if integer_type.sign() == IntegerSign::Signed {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonnegative = Proposition::LessOrEqual(zero, right.clone());
            if semantic_axioms.contains(&nonnegative)
                && let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                    Proposition::LessOrEqual(bound_left, bound_right) => {
                        bound_right == &left
                            && is_minimum_plus(integer_type, bound_left, &right, semantic_axioms)
                    }
                    _ => false,
                })
            {
                return canonical_conjunction(vec![nonnegative, bound.clone()]);
            }
        }
        if integer_type.sign() == IntegerSign::Signed {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonpositive = Proposition::LessOrEqual(right.clone(), zero);
            if semantic_axioms.contains(&nonpositive)
                && let Some(bound) = semantic_axioms.iter().rev().find(|axiom| match axiom {
                    Proposition::LessOrEqual(bound_left, bound_right) => {
                        bound_left == &left
                            && is_maximum_plus(integer_type, bound_right, &right, semantic_axioms)
                    }
                    _ => false,
                })
            {
                return canonical_conjunction(vec![nonpositive, bound.clone()]);
            }
        }
        if let (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(constant))) =
            (integer_type.sign(), known_left)
        {
            if IntegerValue::Unsigned(constant) == integer_type.maximum_value() {
                return Proposition::Truth;
            }
            let boundary = ScalarTerm::integer(integer_type, IntegerValue::Unsigned(constant))
                .expect("known unsigned minuend belongs to its carrier");
            return Proposition::LessOrEqual(right, boundary);
        }
        if integer_type.sign() == IntegerSign::Signed
            && known_left == Some(integer_type.maximum_value())
        {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonnegative = Proposition::LessOrEqual(zero, right.clone());
            if semantic_axioms.contains(&nonnegative) {
                return nonnegative;
            }
        }
        if integer_type.sign() == IntegerSign::Signed
            && known_left == Some(integer_type.minimum_value())
        {
            let zero = ScalarTerm::integer(integer_type, IntegerValue::Signed(0))
                .expect("zero belongs to every signed carrier");
            let nonpositive = Proposition::LessOrEqual(right, zero);
            if semantic_axioms.contains(&nonpositive) {
                return nonpositive;
            }
        }
        return Proposition::Falsehood;
    };
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_then_arithmetic_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_shift_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_then_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_divide_remainder_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_cast_affine_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_chain_then_affine_suffix_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && let Some(constant) = known_right
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_cast_then_offset_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_subtrahend(constant),
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_signed_affine_chain_obligation(
            integer_type,
            left.clone(),
            constant,
            ExactIntegerAffineOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    if known_left.is_none()
        && landed_integer_constant_value(
            integer_type,
            &right,
            semantic_axioms,
            definition_axiom_count,
        ) == Some(constant)
        && let Some(obligation) = exact_integer_mixed_add_subtract_chain_obligation(
            integer_type,
            left.clone(),
            IntegerOffset::from_subtrahend(constant),
            ExactIntegerOffsetOperation::Subtract,
            semantic_axioms,
            definition_axiom_count,
            machine_parameter_values,
        )
    {
        return obligation;
    }
    let original_variable = left.clone();
    let original_offset = IntegerOffset::from_subtrahend(constant);
    let mut variable = left;
    let mut offset = original_offset;
    let mut prior_axiom_count = definition_axiom_count.min(semantic_axioms.len());
    let may_follow_chain =
        landed_integer_constant_value(integer_type, &right, semantic_axioms, prior_axiom_count)
            == Some(constant);
    let mut followed_definition = false;
    if may_follow_chain {
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
            let ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } = definition
            else {
                break;
            };
            if *scalar_type != integer_type {
                break;
            }
            let known_left = landed_integer_constant_value(
                integer_type,
                left,
                semantic_axioms,
                definition_index,
            );
            let Some(nested_constant) = landed_integer_constant_value(
                integer_type,
                right,
                semantic_axioms,
                definition_index,
            ) else {
                break;
            };
            if let Some(base) = known_left {
                let Some(base) = integer_type.exact_sub(base, nested_constant) else {
                    return Proposition::Falsehood;
                };
                let Some(total) = offset.checked_add(IntegerOffset::from_value(base)) else {
                    return Proposition::Falsehood;
                };
                return if total.is_representable(integer_type) {
                    Proposition::Truth
                } else {
                    Proposition::Falsehood
                };
            }
            let Some(combined) =
                offset.checked_add(IntegerOffset::from_subtrahend(nested_constant))
            else {
                return Proposition::Falsehood;
            };
            if combined.magnitude() > integer_type_span(integer_type) {
                return Proposition::Falsehood;
            }
            offset = combined;
            variable = (**left).clone();
            prior_axiom_count = definition_index;
            followed_definition = true;
        }
    }
    if followed_definition
        && !matches!(
            &variable,
            ScalarTerm::Value {
                id,
                scalar_type: ScalarType::Integer(root_type),
            } if *root_type == integer_type && machine_parameter_values.contains(id)
        )
    {
        variable = original_variable;
        offset = original_offset;
    }
    exact_integer_offset_obligation(integer_type, variable, offset)
}

fn exact_integer_mixed_add_subtract_chain_obligation(
    integer_type: psi_core::IntegerType,
    mut variable: ScalarTerm,
    initial_offset: IntegerOffset,
    initial_operation: ExactIntegerOffsetOperation,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Option<Proposition> {
    if integer_type.is_address() || !matches!(integer_type.bits(), 8 | 16 | 32 | 64) {
        return None;
    }
    let mut offset = initial_offset;
    let mut saw_add = initial_operation == ExactIntegerOffsetOperation::Add;
    let mut saw_subtract = initial_operation == ExactIntegerOffsetOperation::Subtract;
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
        let (left, right, operation) = match definition {
            ScalarTerm::ExactIntegerAdd {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => (left, right, ExactIntegerOffsetOperation::Add),
            ScalarTerm::ExactIntegerSubtract {
                scalar_type,
                left,
                right,
            } if *scalar_type == integer_type => {
                (left, right, ExactIntegerOffsetOperation::Subtract)
            }
            _ => break,
        };
        if landed_integer_constant_value(integer_type, left, semantic_axioms, definition_index)
            .is_some()
        {
            break;
        }
        let Some(constant) =
            landed_integer_constant_value(integer_type, right, semantic_axioms, definition_index)
        else {
            break;
        };
        let nested_offset = match operation {
            ExactIntegerOffsetOperation::Add => IntegerOffset::from_value(constant),
            ExactIntegerOffsetOperation::Subtract => IntegerOffset::from_subtrahend(constant),
        };
        let Some(combined) = offset.checked_add(nested_offset) else {
            return Some(Proposition::Falsehood);
        };
        if combined.magnitude() > integer_type_span(integer_type) {
            return Some(Proposition::Falsehood);
        }
        offset = combined;
        variable = (**left).clone();
        prior_axiom_count = definition_index;
        followed_definition = true;
        saw_add |= operation == ExactIntegerOffsetOperation::Add;
        saw_subtract |= operation == ExactIntegerOffsetOperation::Subtract;
    }
    if !followed_definition
        || !saw_add
        || !saw_subtract
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
    Some(exact_integer_offset_obligation(
        integer_type,
        variable,
        offset,
    ))
}

fn is_minimum_plus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    addend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && ((right.as_ref() == addend
            && known_integer_term_value(integer_type, left, semantic_axioms)
                == Some(integer_type.minimum_value()))
            || (left.as_ref() == addend
                && known_integer_term_value(integer_type, right, semantic_axioms)
                    == Some(integer_type.minimum_value())))
}

fn is_maximum_plus(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    addend: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> bool {
    let definition = semantic_axioms
        .iter()
        .rev()
        .find_map(|axiom| match axiom {
            Proposition::Equal(left, right) if left == term => Some(right),
            Proposition::Equal(left, right) if right == term => Some(left),
            _ => None,
        })
        .unwrap_or(term);
    let ScalarTerm::ExactIntegerAdd {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && ((right.as_ref() == addend
            && known_integer_term_value(integer_type, left, semantic_axioms)
                == Some(integer_type.maximum_value()))
            || (left.as_ref() == addend
                && known_integer_term_value(integer_type, right, semantic_axioms)
                    == Some(integer_type.maximum_value())))
}
