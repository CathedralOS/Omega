//! Closed expression-shape validation and Boolean-local distribution for
//! structural scalar returns.

use super::*;

pub(super) fn is_structural_scalar_return_expression(
    expression: &LoweredDirectExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredDirectExpression::Boolean { expression } => {
            is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

pub(super) fn is_structural_boolean_return_expression(
    expression: &LoweredBooleanReturnExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => true,
        LoweredBooleanReturnExpression::Not { operand } => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::Parameter { position } => *position < scalar_parameters,
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { path, .. } => {
            path.len() == 1
        }
        LoweredBooleanReturnExpression::StructuralField { .. } => true,
        LoweredBooleanReturnExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
    }
}

pub(super) fn is_branch_free_structural_integer_expression(
    expression: &LoweredDirectExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredDirectExpression::IntegerLiteral { .. } => true,
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. }
        | LoweredDirectExpression::IntegerWiden { operand, .. }
        | LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            is_branch_free_structural_integer_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        LoweredDirectExpression::Parameter { position, .. } => *position < scalar_parameters,
        LoweredDirectExpression::Local { position, .. } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        LoweredDirectExpression::Boolean { .. } => false,
    }
}

pub(super) fn is_branch_free_structural_scalar_expression(
    expression: &LoweredDirectExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredDirectExpression::Boolean { expression } => {
            is_branch_free_structural_boolean_expression(
                expression,
                scalar_parameters,
                available_locals,
            )
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

pub(super) fn is_branch_free_structural_boolean_expression(
    expression: &LoweredBooleanReturnExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => true,
        LoweredBooleanReturnExpression::Not { operand } => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        LoweredBooleanReturnExpression::Parameter { position } => *position < scalar_parameters,
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { path, .. } => {
            path.len() == 1
        }
        LoweredBooleanReturnExpression::StructuralField { .. } => true,
        LoweredBooleanReturnExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            false
        }
    }
}

pub(super) fn is_structural_short_circuit_boolean_decision(
    expression: &LoweredBooleanReturnExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    contains_short_circuit(expression)
        && is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
}

pub(super) fn boolean_local_reference_count(
    expression: &LoweredBooleanReturnExpression,
    local: usize,
) -> usize {
    match expression {
        LoweredBooleanReturnExpression::Local { position } => usize::from(*position == local),
        LoweredBooleanReturnExpression::Not { operand } => {
            boolean_local_reference_count(operand, local)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            boolean_local_reference_count(left, local)
                .saturating_add(boolean_local_reference_count(right, local))
        }
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => 0,
    }
}

pub(super) fn inline_boolean_local(
    expression: &LoweredBooleanReturnExpression,
    local: usize,
    replacement: &LoweredBooleanReturnExpression,
) -> LoweredBooleanReturnExpression {
    match expression {
        LoweredBooleanReturnExpression::Local { position } if *position == local => {
            replacement.clone()
        }
        LoweredBooleanReturnExpression::Not { operand } => LoweredBooleanReturnExpression::Not {
            operand: Box::new(inline_boolean_local(operand, local, replacement)),
        },
        LoweredBooleanReturnExpression::Equal { left, right } => {
            LoweredBooleanReturnExpression::Equal {
                left: Box::new(inline_boolean_local(left, local, replacement)),
                right: Box::new(inline_boolean_local(right, local, replacement)),
            }
        }
        LoweredBooleanReturnExpression::And { left, right } => {
            LoweredBooleanReturnExpression::And {
                left: Box::new(inline_boolean_local(left, local, replacement)),
                right: Box::new(inline_boolean_local(right, local, replacement)),
            }
        }
        LoweredBooleanReturnExpression::Or { left, right } => LoweredBooleanReturnExpression::Or {
            left: Box::new(inline_boolean_local(left, local, replacement)),
            right: Box::new(inline_boolean_local(right, local, replacement)),
        },
        expression => expression.clone(),
    }
}

pub(super) fn source_distribute_boolean_local(
    decision: LoweredBooleanDecision,
    continuation: &LoweredBooleanReturnExpression,
    local: usize,
) -> LoweredBooleanDecision {
    // Preserve source evaluation exactly once: decide the staged value first,
    // then substitute only its already-computed leaf into each pure
    // continuation copy. Replacing every use with the original decision tree
    // would duplicate both execution and logical fuel.
    bind_boolean_decision(decision, &|value| {
        lower_boolean_value_decision(&inline_boolean_local(continuation, local, value))
    })
}

pub(super) fn validate_boolean_decision_parameter_types(
    decision: &LoweredBooleanDecision,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match decision {
        LoweredBooleanDecision::Value(expression) => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
        LoweredBooleanDecision::Test {
            condition,
            when_true,
            when_false,
        } => {
            validate_boolean_parameter_types(condition, parameter_types)?;
            validate_boolean_decision_parameter_types(when_true, parameter_types)?;
            validate_boolean_decision_parameter_types(when_false, parameter_types)
        }
    }
}
