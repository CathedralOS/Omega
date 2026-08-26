//! Leaf divide/remainder reduction strategies.
//!
//! These sufficient-form reducers remain explicit trusted migration
//! dependencies until they prove the canonical semantic-ledger goals with
//! checked certificates. They do not own artifact traversal or goal identity.

use std::collections::BTreeSet;

use psi_core::{IntegerSign, IntegerValue, Proposition, PropositionContext, ScalarTerm, ValueId};

use super::affine_joins::exact_integer_same_root_affine_divide_remainder_join_obligation;
use super::{canonical_conjunction, known_integer_term_value};

pub(super) fn is_maximum_divide(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    divisor: &ScalarTerm,
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
    let ScalarTerm::ExactIntegerDivide {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == divisor
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.maximum_value())
}

pub(super) fn is_minimum_divide(
    integer_type: psi_core::IntegerType,
    term: &ScalarTerm,
    divisor: &ScalarTerm,
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
    let ScalarTerm::ExactIntegerDivide {
        scalar_type,
        left,
        right,
    } = definition
    else {
        return false;
    };
    *scalar_type == integer_type
        && right.as_ref() == divisor
        && known_integer_term_value(integer_type, left, semantic_axioms)
            == Some(integer_type.minimum_value())
}

pub(super) fn exact_integer_divide_obligation_with_definitions(
    proposition_context: &PropositionContext,
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    if known_integer_term_value(integer_type, &left, semantic_axioms).is_none()
        && known_integer_term_value(integer_type, &right, semantic_axioms).is_none()
        && let Some(obligation) = exact_integer_same_root_affine_divide_remainder_join_obligation(
            proposition_context,
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
    exact_integer_divide_obligation(integer_type, left, right, semantic_axioms)
}

pub(super) fn exact_integer_divide_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_div(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_))) => Proposition::Truth,
        (IntegerSign::Signed, Some(IntegerValue::Signed(-1))) => {
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
            .expect("exact-divide negation boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, left)
        }
        (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, false),
    }
}

pub(super) fn exact_integer_remainder_obligation_with_definitions(
    proposition_context: &PropositionContext,
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    definition_axiom_count: usize,
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    if known_integer_term_value(integer_type, &left, semantic_axioms).is_none()
        && known_integer_term_value(integer_type, &right, semantic_axioms).is_none()
        && let Some(obligation) = exact_integer_same_root_affine_divide_remainder_join_obligation(
            proposition_context,
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
    exact_integer_remainder_obligation(integer_type, left, right, semantic_axioms)
}

pub(super) fn exact_integer_remainder_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.exact_rem(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_))) => Proposition::Truth,
        (IntegerSign::Signed, Some(IntegerValue::Signed(-1))) => {
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed type has signed minimum")
            };
            let boundary = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            )
            .expect("exact-remainder boundary remains in the carrier");
            Proposition::LessOrEqual(boundary, left)
        }
        (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, false),
    }
}

#[cfg(test)]
pub(super) fn wrapping_integer_divide_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.wrapping_div(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

#[cfg(test)]
pub(super) fn wrapping_integer_remainder_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.wrapping_rem(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

#[cfg(test)]
pub(super) fn saturating_integer_divide_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.saturating_div(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

#[cfg(test)]
pub(super) fn saturating_integer_remainder_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Proposition {
    let known_left = known_integer_term_value(integer_type, &left, semantic_axioms);
    let known_right = known_integer_term_value(integer_type, &right, semantic_axioms);
    if let (Some(left), Some(right)) = (known_left, known_right) {
        return if integer_type.saturating_rem(left, right).is_some() {
            Proposition::Truth
        } else {
            Proposition::Falsehood
        };
    }
    match (integer_type.sign(), known_right) {
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(0)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(0))) => Proposition::Falsehood,
        (IntegerSign::Unsigned, Some(IntegerValue::Unsigned(_)))
        | (IntegerSign::Signed, Some(IntegerValue::Signed(_))) => Proposition::Truth,
        _ => runtime_divisor_obligation(integer_type, left, right, semantic_axioms, true),
    }
}

fn runtime_divisor_obligation(
    integer_type: psi_core::IntegerType,
    left: ScalarTerm,
    right: ScalarTerm,
    semantic_axioms: &[Proposition],
    negative_one_is_total: bool,
) -> Proposition {
    if integer_type.sign() == IntegerSign::Signed {
        let negative_one = ScalarTerm::integer(integer_type, IntegerValue::Signed(-1))
            .expect("every signed fixed integer carrier admits negative one");
        let negative_one_bound = Proposition::LessOrEqual(right.clone(), negative_one);
        if semantic_axioms.contains(&negative_one_bound) {
            if negative_one_is_total {
                return negative_one_bound;
            }
            let IntegerValue::Signed(minimum) = integer_type.minimum_value() else {
                unreachable!("signed fixed integer has a signed minimum")
            };
            if let Ok(minimum_plus_one) = ScalarTerm::integer(
                integer_type,
                IntegerValue::Signed(minimum.checked_add(1).expect("minimum has a successor")),
            ) {
                let dividend_bound = Proposition::LessOrEqual(minimum_plus_one, left);
                if semantic_axioms.contains(&dividend_bound) {
                    return canonical_conjunction(vec![negative_one_bound, dividend_bound]);
                }
            }
        }
    }
    if integer_type.sign() == IntegerSign::Signed {
        if let Ok(negative_two) = ScalarTerm::integer(integer_type, IntegerValue::Signed(-2)) {
            let negative_bound = Proposition::LessOrEqual(right.clone(), negative_two);
            if semantic_axioms.contains(&negative_bound) {
                return negative_bound;
            }
        }
    }
    let one = match integer_type.sign() {
        IntegerSign::Unsigned => IntegerValue::Unsigned(1),
        IntegerSign::Signed => IntegerValue::Signed(1),
    };
    let Ok(boundary) = ScalarTerm::integer(integer_type, one) else {
        return Proposition::Falsehood;
    };
    Proposition::LessOrEqual(boundary, right)
}
