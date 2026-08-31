//! Closed and parameter-backed guards admitted by composed Unit control.

use super::*;

pub(super) fn exact_guard(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::Boolean(boolean) = expression else {
        return None;
    };
    match (scalar_parameters, boolean.as_ref()) {
        ([parameter], psi_checked_trees::CheckedBooleanExpression::Parameter { position: 0 })
            if parameter.source_position == 0
                && parameter.primitive_type == PrimitiveType::Bool =>
        {
            Some(expression.clone())
        }
        ([], boolean) if closed_boolean(boolean) => Some(expression.clone()),
        _ => None,
    }
}

fn closed_boolean(expression: &psi_checked_trees::CheckedBooleanExpression) -> bool {
    match expression {
        psi_checked_trees::CheckedBooleanExpression::Constant(_) => true,
        psi_checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            matches!(
                left.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            ) && matches!(
                right.as_ref(),
                CheckedScalarExpression::IntegerLiteral { .. }
            )
        }
        _ => false,
    }
}
