//! Closed and parameter-backed guards admitted by composed Unit control.

use super::super::{
    CheckedScalarBinding, CheckedScalarBindingValue, CheckedScalarExpression,
    CheckedStructuralScalarParameterPlan, PrimitiveType,
};

pub(super) fn exact_guard(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
    bindings: &[CheckedScalarBinding],
) -> Option<CheckedScalarExpression> {
    let CheckedScalarExpression::Boolean(boolean) = expression else {
        return None;
    };
    match (bindings, boolean.as_ref()) {
        // The joined guard evaluates at the caller: the scalar parameters,
        // a retained `self` field, and literal constants are its admitted
        // operand roots.
        ([], shape) if admitted_guard_shape(shape, scalar_parameters) => Some(expression.clone()),
        ([], shape) if scalar_parameters.is_empty() && closed_boolean(shape) => {
            Some(expression.clone())
        }
        (
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

/// Guard shapes the joined caller can evaluate itself: a Boolean parameter
/// operand, a retained `self` field, equality or integer-equality over the
/// admitted operand roots, and negations of those. Everything else —
/// locals, calls, and ordering comparisons — still declines.
fn admitted_guard_shape(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Parameter { position } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool),
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => retained_field_subject(*parameter_position, path),
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            (boolean_subject(left) || boolean_subject(right))
                && boolean_operand(left, scalar_parameters)
                && boolean_operand(right, scalar_parameters)
        }
        checked_trees::CheckedBooleanExpression::Not(inner) => {
            admitted_guard_shape(inner, scalar_parameters)
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison {
            kind: checked_trees::CheckedIntegerComparisonKind::Equal,
            left,
            right,
        } => {
            (integer_subject(left) || integer_subject(right))
                && integer_operand(left, scalar_parameters)
                && integer_operand(right, scalar_parameters)
        }
        _ => false,
    }
}

/// A retained-field read rooted at an authored structural parameter no
/// deeper than the implicit `self` slot, walking record fields only; case
/// and index segments decline. Authored structural parameters still decline
/// at the unit-topology roster, so position 0 is what reaches here today.
fn retained_field_subject(
    parameter_position: u32,
    path: &[checked_trees::CheckedStructuralPredicatePathSegment],
) -> bool {
    parameter_position <= 1
        && !path.is_empty()
        && path.iter().all(|segment| {
            matches!(
                segment,
                checked_trees::CheckedStructuralPredicatePathSegment::Field(_)
            )
        })
}

/// One Boolean operand of a joined equality: a Boolean parameter, a
/// retained `self` field, or a Boolean literal.
fn boolean_operand(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::Parameter { position } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == PrimitiveType::Bool),
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::StructuralParameterField {
            parameter_position,
            path,
        } => retained_field_subject(*parameter_position, path),
        _ => false,
    }
}

/// One integer operand of a joined equality: a scalar parameter (typed to
/// match it), a retained `self` field, or an integer literal.
fn integer_operand(
    expression: &CheckedScalarExpression,
    scalar_parameters: &[CheckedStructuralScalarParameterPlan],
) -> bool {
    match expression {
        CheckedScalarExpression::Parameter {
            position,
            primitive_type,
        } => scalar_parameters
            .get(*position)
            .is_some_and(|parameter| parameter.primitive_type == *primitive_type),
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::StructuralParameterField {
            parameter_position,
            path,
            ..
        } => retained_field_subject(*parameter_position, path),
        _ => false,
    }
}

/// Whether an equality operand names a runtime subject — a joined
/// parameter or a retained field — rather than a pair of literals.
fn boolean_subject(expression: &checked_trees::CheckedBooleanExpression) -> bool {
    matches!(
        expression,
        checked_trees::CheckedBooleanExpression::Parameter { .. }
            | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
    )
}

fn integer_subject(expression: &CheckedScalarExpression) -> bool {
    matches!(
        expression,
        CheckedScalarExpression::Parameter { .. }
            | CheckedScalarExpression::StructuralParameterField { .. }
    )
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
