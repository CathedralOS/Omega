//! Known results of freshly built computation DAGs, not permission to erase them.
//!
//! Calls and runtime storage stay unknown. A selection can nevertheless have
//! one result on every normal-return path. Inspecting both branches here executes
//! neither; the original graph still owns effects, failures, and evaluation order.
//! Only Boolean selection and negation compose these results, matching flow's
//! control facts. Strict operators still require closed scalar evaluation;
//! interpreting a call-bearing comparison here would prune calls flow retained.

use super::*;
use facts::ScalarValue;

pub(super) fn boolean_result(
    plans: &CheckedScalarComputationPlans,
    computation: CheckedScalarComputationHandle,
) -> Option<bool> {
    match &plans.nodes.get(computation).kind {
        CheckedScalarComputationKind::SelectedComparison { .. } => None,
        CheckedScalarComputationKind::Qualification { operand, .. } => {
            boolean_result(plans, *operand)
        }
        CheckedScalarComputationKind::Value(CheckedScalarExpression::Boolean(value)) => {
            boolean_expression_result(value)
        }
        CheckedScalarComputationKind::Value(_) => None,
        CheckedScalarComputationKind::Call { .. } => None,
        CheckedScalarComputationKind::Dispatch { .. } => None,
        CheckedScalarComputationKind::Select {
            condition,
            when_true,
            when_false,
            ..
        } => match boolean_result(plans, *condition) {
            Some(true) => boolean_result(plans, *when_true),
            Some(false) => boolean_result(plans, *when_false),
            None => {
                let when_true = boolean_result(plans, *when_true)?;
                let when_false = boolean_result(plans, *when_false)?;
                (when_true == when_false).then_some(when_true)
            }
        },
        CheckedScalarComputationKind::Apply {
            expression,
            operands,
            ..
        } => {
            let CheckedScalarExpression::Boolean(value) = expression else {
                return None;
            };
            let CheckedBooleanExpression::Not(operand) = value.as_ref() else {
                return None;
            };
            let [operand_computation] = plans.operands.span(*operands)? else {
                return None;
            };
            if !matches!(
                operand.as_ref(),
                CheckedBooleanExpression::Parameter { position: 0 }
            ) {
                return None;
            }
            Some(!boolean_result(plans, *operand_computation)?)
        }
    }
}

fn boolean_expression_result(expression: &CheckedBooleanExpression) -> Option<bool> {
    match expression {
        CheckedBooleanExpression::Not(operand) => Some(!boolean_expression_result(operand)?),
        CheckedBooleanExpression::And { left, right }
        | CheckedBooleanExpression::Or { left, right } => {
            let evaluate_when = matches!(expression, CheckedBooleanExpression::And { .. });
            let left = boolean_expression_result(left);
            if left == Some(!evaluate_when) {
                return left;
            }
            let right = boolean_expression_result(right);
            if left == Some(evaluate_when) || right == Some(!evaluate_when) {
                right
            } else {
                None
            }
        }
        _ => match crate::values::evaluate_checked_scalar(
            &CheckedScalarExpression::Boolean(Box::new(expression.clone())),
            &mut |_| None,
        )? {
            ScalarValue::Boolean(value) => Some(value),
            _ => None,
        },
    }
}
