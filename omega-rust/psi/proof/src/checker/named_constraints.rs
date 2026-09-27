//! Named-constraint obligations and the constraints that satisfy them.

use crate::checker::diagnostics::{
    cannot_prove_assignment_named_constraint, cannot_prove_call_named_constraint,
    cannot_prove_initializer_named_constraint, cannot_prove_return_named_constraint,
    cannot_prove_transition_named_constraint,
};
use crate::checker::float_ranges::{finite_float_literal, float_range_from_constraints};
use crate::checker::integer_ranges::{
    guarded_integer_range_for_assignment, guarded_integer_range_for_transition_argument,
    integer_range_for_transition_argument, integer_range_from_constraints, type_constraints,
};
use crate::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, ProofConstraint, ProofPlan,
};
use arena::HandleSpan;
use diagnostics::Diagnostic;
use typed_trees::expression::{ExpressionHandle, ExpressionNode};

pub(crate) fn check_call_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.argument,
            obligation.argument_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_call_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

pub(crate) fn check_assignment_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !assignment_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_assignment_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

pub(crate) fn check_transition_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !transition_argument_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_transition_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

pub(crate) fn check_return_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !argument_handle_satisfies_named_constraint(
            proof_plan,
            obligation.value,
            obligation.value_constraints,
            constraint,
        ) {
            diagnostics.push(cannot_prove_return_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

pub(crate) fn check_initializer_named_constraints(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    diagnostics: &mut Vec<Diagnostic>,
) {
    for constraint in named_constraints(type_constraints(proof_plan, obligation.constraints)) {
        if !initializer_satisfies_named_constraint(proof_plan, obligation, constraint) {
            diagnostics.push(cannot_prove_initializer_named_constraint(
                proof_plan, obligation, constraint,
            ));
        }
    }
}

fn named_constraints(constraints: &[ProofConstraint]) -> impl Iterator<Item = &str> {
    constraints.iter().filter_map(|constraint| {
        let ProofConstraint::Named(name) = constraint else {
            return None;
        };

        Some(name.as_str())
    })
}

fn argument_handle_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    argument: ExpressionHandle,
    argument_constraints: HandleSpan<ProofConstraint>,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, argument_constraints);

    constraints_satisfy_named_constraint(constraints, constraint)
        || match (
            constraint,
            proof_plan.program.expression_table.expression(argument),
        ) {
            ("exact", ExpressionNode::Integer(_)) => true,
            ("finite", ExpressionNode::Float(value)) => finite_float_literal(value).is_some(),
            ("finite", ExpressionNode::Integer(_)) => true,
            ("non_negative", ExpressionNode::Integer(value)) => {
                value.value_i64().is_some_and(|value| value >= 0)
            }
            ("positive", ExpressionNode::Integer(value)) => {
                value.value_i64().is_some_and(|value| value > 0)
            }
            ("wrapping", ExpressionNode::Integer(_)) => true,
            _ => false,
        }
}

fn transition_argument_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, obligation.argument_constraints);

    if constraints_satisfy_named_constraint(constraints, constraint) {
        return true;
    }

    if matches!(constraint, "positive" | "non_negative") {
        let range = guarded_integer_range_for_transition_argument(proof_plan, obligation);
        return match constraint {
            "positive" => !range.minimum.is_negative() && !range.minimum.is_zero(),
            "non_negative" => !range.minimum.is_negative(),
            _ => false,
        };
    }

    if matches!(constraint, "exact")
        && integer_range_for_transition_argument(proof_plan, obligation)
            .is_some_and(|range| range.minimum == range.maximum)
    {
        return true;
    }

    argument_handle_satisfies_named_constraint(
        proof_plan,
        obligation.argument,
        obligation.argument_constraints,
        constraint,
    )
}

fn assignment_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
    constraint: &str,
) -> bool {
    let constraints = type_constraints(proof_plan, obligation.value_constraints);

    if constraints_satisfy_named_constraint(constraints, constraint) {
        return true;
    }

    if matches!(constraint, "positive" | "non_negative")
        && let Some(range) = guarded_integer_range_for_assignment(proof_plan, obligation)
    {
        return match constraint {
            "positive" => !range.minimum.is_negative() && !range.minimum.is_zero(),
            "non_negative" => !range.minimum.is_negative(),
            _ => false,
        };
    }

    if matches!(constraint, "exact")
        && guarded_integer_range_for_assignment(proof_plan, obligation)
            .is_some_and(|range| range.minimum == range.maximum)
    {
        return true;
    }

    argument_handle_satisfies_named_constraint(
        proof_plan,
        obligation.value,
        obligation.value_constraints,
        constraint,
    )
}

fn initializer_satisfies_named_constraint(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
    constraint: &str,
) -> bool {
    match (
        constraint,
        proof_plan
            .program
            .expression_table
            .expression(obligation.value),
    ) {
        ("finite", ExpressionNode::Float(value)) => finite_float_literal(value).is_some(),
        ("exact", ExpressionNode::Integer(_)) => true,
        ("non_negative", ExpressionNode::Integer(value)) => {
            value.value_i64().is_some_and(|value| value >= 0)
        }
        ("positive", ExpressionNode::Integer(value)) => {
            value.value_i64().is_some_and(|value| value > 0)
        }
        ("wrapping", ExpressionNode::Integer(_)) => true,
        _ => false,
    }
}

fn constraints_satisfy_named_constraint(constraints: &[ProofConstraint], constraint: &str) -> bool {
    if constraints.iter().any(|argument_constraint| {
        matches!(
            argument_constraint,
            ProofConstraint::Named(argument_constraint)
                if argument_constraint.as_str() == constraint
        )
    }) {
        return true;
    }

    match constraint {
        "exact" => integer_range_from_constraints(constraints).is_some(),
        // A float range satisfies `finite` only when its membership excludes
        // the infinities too: an inclusive +inf endpoint admits +inf, so
        // range EXISTENCE alone is not evidence.
        "finite" => {
            integer_range_from_constraints(constraints).is_some()
                || float_range_from_constraints(constraints)
                    .is_some_and(|range| range.proves_finite())
        }
        "non_negative" => integer_range_from_constraints(constraints)
            .is_some_and(|range| !range.minimum.is_negative()),
        "positive" => integer_range_from_constraints(constraints)
            .is_some_and(|range| !range.minimum.is_negative() && !range.minimum.is_zero()),
        "wrapping" => false,
        _ => false,
    }
}
