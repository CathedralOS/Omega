//! Closed and parameter-backed guards admitted by composed Unit control.

use super::*;

pub(super) fn exact_guard(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    bindings: &[CheckedScalarBinding],
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::Boolean(boolean) = expression else {
        return None;
    };
    match (scalar_parameters, bindings, boolean.as_ref()) {
        ([parameter], [], checked_trees::CheckedBooleanExpression::Parameter { position: 0 })
            if parameter.source_position <= 1
                && parameter.primitive_type == PrimitiveType::Bool =>
        {
            Some(expression.clone())
        }
        ([], [], boolean) if closed_boolean(boolean) => Some(expression.clone()),
        (
            [],
            [
                CheckedScalarBinding {
                    destination: checked_trees::CheckedScalarBindingDestination::Immutable,
                    statement_ordinal: 0,
                    primitive_type: PrimitiveType::U64,
                    value: CheckedScalarBindingValue::Expression,
                },
            ],
            checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. },
        ) if local_and_literal(left, right) || local_and_literal(right, left) => {
            Some(expression.clone())
        }
        _ => None,
    }
}

fn local_and_literal(local: &CheckedScalarExpression, literal: &CheckedScalarExpression) -> bool {
    matches!(
        local,
        CheckedScalarExpression::Local {
            position: 0,
            primitive_type: PrimitiveType::U64,
        }
    ) && matches!(literal, CheckedScalarExpression::IntegerLiteral { .. })
}

fn closed_boolean(expression: &checked_trees::CheckedBooleanExpression) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
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
