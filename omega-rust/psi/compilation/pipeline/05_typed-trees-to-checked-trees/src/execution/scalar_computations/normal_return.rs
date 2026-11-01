//! Known results of freshly built computation DAGs, not permission to erase them.
//!
//! Calls and runtime storage stay unknown. A selection can nevertheless have
//! one result on every normal-return path. Inspecting both branches here executes
//! neither; the original graph still owns effects, failures, and evaluation order.
//! Only Boolean selection and negation compose these results, matching flow's
//! control facts. Strict operators still require closed scalar evaluation;
//! interpreting a call-bearing comparison here would prune calls flow retained.
use super::{
    CheckedBooleanExpression as BooleanExpression, CheckedScalarComputationHandle,
    CheckedScalarComputationKind as ComputationKind, CheckedScalarComputationPlans,
    CheckedScalarExpression as ScalarExpression,
};
use facts::ScalarValue;

pub(super) fn boolean_result(
    plans: &CheckedScalarComputationPlans,
    computation: CheckedScalarComputationHandle,
) -> Option<bool> {
    match &plans.nodes.get(computation).kind {
        ComputationKind::WithOperands { .. }
        | ComputationKind::StorageObservation { .. }
        | ComputationKind::SelectedComparison { .. }
        | ComputationKind::StorageAddress { .. }
        | ComputationKind::StructuralField { .. }
        | ComputationKind::ByteSequenceLiteralComparison { .. }
        | ComputationKind::ByteSequencePredicate { .. }
        | ComputationKind::CaseMembership { .. }
        | ComputationKind::BooleanToInteger { .. }
        | ComputationKind::AuthoredCast { .. } => None,
        ComputationKind::Qualification { operand, .. } => boolean_result(plans, *operand),
        ComputationKind::Value(ScalarExpression::Boolean(value)) => {
            boolean_expression_result(value)
        }
        ComputationKind::RetainedConstant { .. } | ComputationKind::Value(_) => None,
        ComputationKind::Call { .. } => None,
        ComputationKind::Dispatch { .. } => None,
        ComputationKind::Select {
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
        ComputationKind::Apply {
            expression,
            operands,
            ..
        } => {
            let ScalarExpression::Boolean(value) = expression else {
                return None;
            };
            let BooleanExpression::Not(operand) = value.as_ref() else {
                return None;
            };
            let [operand_computation] = plans.operands.span(*operands)? else {
                return None;
            };
            if !matches!(
                operand.as_ref(),
                BooleanExpression::Parameter { position: 0 }
            ) {
                return None;
            }
            Some(!boolean_result(plans, *operand_computation)?)
        }
    }
}

fn boolean_expression_result(expression: &BooleanExpression) -> Option<bool> {
    match expression {
        BooleanExpression::Not(operand) => Some(!boolean_expression_result(operand)?),
        BooleanExpression::And { left, right } | BooleanExpression::Or { left, right } => {
            let evaluate_when = matches!(expression, BooleanExpression::And { .. });
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
            &ScalarExpression::Boolean(Box::new(expression.clone())),
            &mut |_| None,
        )? {
            ScalarValue::Boolean(value) => Some(value),
            _ => None,
        },
    }
}
