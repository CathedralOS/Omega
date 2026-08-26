//! Legacy sufficient-form reduction dispatch.
//!
//! The canonical proof-bearing scalar goal is selected by
//! `psi-terminal-semantics`. This module keeps the current migration behavior:
//! it asks one trusted Rust reducer for a proposition sufficient to discharge
//! that goal. The reducers must eventually move behind a certificate-producing
//! boundary and prove the unchanged canonical goal.

use std::collections::BTreeSet;

use psi_core::{Proposition, PropositionContext, ScalarTerm, ValueId};
use psi_terminal_semantics::{
    CanonicalScalarGoal, ProofBearingScalarLeafSemantics, ScalarLeafDenotation,
};

use super::integer_add_subtract::{
    exact_integer_add_obligation, exact_integer_subtract_obligation,
};
use super::integer_conversion::exact_integer_cast_obligation;
use super::integer_divide_remainder::{
    exact_integer_divide_obligation_with_definitions,
    exact_integer_remainder_obligation_with_definitions,
};
use super::integer_multiply::exact_integer_multiply_obligation_with_definitions;
use super::integer_shift::{exact_integer_shift_left_obligation, exact_integer_shift_obligation};

pub(super) fn reduce_proof_bearing_scalar_goal(
    proposition_context: &PropositionContext,
    semantics: &ProofBearingScalarLeafSemantics,
    semantic_axioms: &[Proposition],
    machine_requirements: &[Proposition],
    machine_parameter_values: &BTreeSet<ValueId>,
) -> Proposition {
    let definition_axiom_count = semantic_axioms.len();
    let available_bounds = || {
        let mut available = semantic_axioms.to_vec();
        available.extend_from_slice(machine_requirements);
        available
    };
    match (semantics.schema().denotation(), semantics.canonical_goal()) {
        (
            ScalarLeafDenotation::IntegerExactCast,
            CanonicalScalarGoal::ExactCastRepresentable {
                source_type,
                target_type,
                operand,
            },
        ) => exact_integer_cast_obligation(
            *source_type,
            *target_type,
            operand.clone(),
            semantic_axioms,
            machine_parameter_values,
        ),
        (
            ScalarLeafDenotation::ExactIntegerShiftRight,
            CanonicalScalarGoal::ExactShiftCount {
                value_type,
                count_type,
                count,
            },
        ) => {
            exact_integer_shift_obligation(*value_type, *count_type, count.clone(), semantic_axioms)
        }
        (
            ScalarLeafDenotation::ExactIntegerShiftLeft,
            CanonicalScalarGoal::ExactShiftLeftRepresentable {
                value_type,
                count_type,
                value,
                count,
            },
        ) => exact_integer_shift_left_obligation(
            *value_type,
            *count_type,
            value.clone(),
            count.clone(),
            &available_bounds(),
            definition_axiom_count,
            machine_parameter_values,
        ),
        (
            ScalarLeafDenotation::ExactIntegerAdd,
            CanonicalScalarGoal::ExactArithmeticRepresentable { integer_type, .. },
        ) => {
            let ScalarTerm::ExactIntegerAdd { left, right, .. } = semantics.denotation() else {
                unreachable!("canonical exact-add denotation retains exact operands")
            };
            exact_integer_add_obligation(
                *integer_type,
                (**left).clone(),
                (**right).clone(),
                &available_bounds(),
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        (
            ScalarLeafDenotation::ExactIntegerSubtract,
            CanonicalScalarGoal::ExactArithmeticRepresentable { integer_type, .. },
        ) => {
            let ScalarTerm::ExactIntegerSubtract { left, right, .. } = semantics.denotation()
            else {
                unreachable!("canonical exact-subtract denotation retains exact operands")
            };
            exact_integer_subtract_obligation(
                *integer_type,
                (**left).clone(),
                (**right).clone(),
                &available_bounds(),
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        (
            ScalarLeafDenotation::ExactIntegerMultiply,
            CanonicalScalarGoal::ExactArithmeticRepresentable { integer_type, .. },
        ) => {
            let ScalarTerm::ExactIntegerMultiply { left, right, .. } = semantics.denotation()
            else {
                unreachable!("canonical exact-multiply denotation retains exact operands")
            };
            exact_integer_multiply_obligation_with_definitions(
                *integer_type,
                (**left).clone(),
                (**right).clone(),
                &available_bounds(),
                definition_axiom_count,
                machine_parameter_values,
            )
        }
        (
            ScalarLeafDenotation::ExactIntegerDivide,
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type,
                left,
                right,
            },
        ) => exact_integer_divide_obligation_with_definitions(
            proposition_context,
            *integer_type,
            left.clone(),
            right.clone(),
            &available_bounds(),
            definition_axiom_count,
            machine_parameter_values,
        ),
        (
            ScalarLeafDenotation::ExactIntegerRemainder,
            CanonicalScalarGoal::ExactDivisionDefined {
                integer_type,
                left,
                right,
            },
        ) => exact_integer_remainder_obligation_with_definitions(
            proposition_context,
            *integer_type,
            left.clone(),
            right.clone(),
            &available_bounds(),
            definition_axiom_count,
            machine_parameter_values,
        ),
        (
            ScalarLeafDenotation::WrappingIntegerDivide,
            CanonicalScalarGoal::NonzeroDivisor { .. },
        )
        | (
            ScalarLeafDenotation::WrappingIntegerRemainder,
            CanonicalScalarGoal::NonzeroDivisor { .. },
        )
        | (
            ScalarLeafDenotation::SaturatingIntegerDivide,
            CanonicalScalarGoal::NonzeroDivisor { .. },
        )
        | (
            ScalarLeafDenotation::SaturatingIntegerRemainder,
            CanonicalScalarGoal::NonzeroDivisor { .. },
        ) => unreachable!("canonical nonzero rows bypass legacy sufficient reduction"),
        _ => unreachable!("validated proof-bearing scalar row has one canonical goal mapping"),
    }
}
