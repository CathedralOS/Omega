//! Float ranges proved for the operands each bounded obligation names.

use crate::proof_engine::checker::integer_ranges::type_constraints;
use crate::proof_engine::obligations::{
    BoundedAssignmentObligation, BoundedCallArgumentObligation, BoundedInitializerObligation,
    BoundedStateReturnObligation, BoundedTransitionArgumentObligation, FloatRange, ProofConstraint,
    ProofPlan,
};
use symbol_resolved_trees_to_typed_trees::typed_trees::expression::ExpressionNode;

pub(crate) fn float_range_for_transition_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedTransitionArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

pub(crate) fn float_range_for_assignment(
    proof_plan: &ProofPlan,
    obligation: &BoundedAssignmentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }
    }
}

pub(crate) fn float_range_for_call_argument(
    proof_plan: &ProofPlan,
    obligation: &BoundedCallArgumentObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.argument)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => float_range_from_constraints(type_constraints(
            proof_plan,
            obligation.argument_constraints,
        )),
    }
}

pub(crate) fn float_range_for_return_value(
    proof_plan: &ProofPlan,
    obligation: &BoundedStateReturnObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => {
            float_range_from_constraints(type_constraints(proof_plan, obligation.value_constraints))
        }
    }
}

pub(crate) fn float_range_for_initializer(
    proof_plan: &ProofPlan,
    obligation: &BoundedInitializerObligation,
) -> Option<FloatRange> {
    match proof_plan
        .program
        .expression_table
        .expression(obligation.value)
    {
        ExpressionNode::Float(value) => {
            let value = finite_float_literal(value)?;
            Some(FloatRange::closed(value))
        }
        _ => None,
    }
}

pub(crate) fn float_range_from_constraints(constraints: &[ProofConstraint]) -> Option<FloatRange> {
    let mut range: Option<FloatRange> = None;

    for constraint in constraints {
        let ProofConstraint::FloatRange {
            minimum,
            maximum,
            maximum_inclusive,
        } = constraint
        else {
            continue;
        };

        let candidate = FloatRange {
            minimum: minimum.landed_f64(),
            maximum: maximum.landed_f64(),
            maximum_inclusive: *maximum_inclusive,
        };

        range = Some(match range {
            Some(existing) => existing.intersect(candidate),
            None => candidate,
        });
    }

    range
}

pub(crate) fn finite_float_literal(
    value: &symbol_resolved_trees_to_typed_trees::typed_trees::expression::FloatLiteral,
) -> Option<f64> {
    // The riding landing decides the value: `0.3f32` is the widened f32, not
    // the f64 read of its spelling (and f32 overflow is genuinely infinite).
    let value = value.landed_f64();
    value.is_finite().then_some(value)
}

pub(crate) fn float_range_display(range: FloatRange) -> String {
    if range.maximum_inclusive {
        format!("expected {}..={}", range.minimum, range.maximum)
    } else {
        format!("expected {}..{}", range.minimum, range.maximum)
    }
}
