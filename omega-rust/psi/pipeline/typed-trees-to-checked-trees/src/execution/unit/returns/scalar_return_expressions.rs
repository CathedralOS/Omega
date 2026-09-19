//! Classification of scalar and boolean return expressions.

use crate::execution::terminal_unit::CheckedScalarExpression;

pub(crate) fn checked_boolean_contains_short_circuit(
    expression: &checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::And { .. }
        | checked_trees::CheckedBooleanExpression::Or { .. } => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | checked_trees::CheckedBooleanExpression::Local { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}

pub(crate) fn is_structural_short_circuit_boolean_return(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    let CheckedScalarExpression::Boolean(expression) = expression else {
        return false;
    };
    checked_boolean_contains_short_circuit(expression)
        && is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
}

pub(crate) fn checked_boolean_local_reference_count(
    expression: &checked_trees::CheckedBooleanExpression,
    local: usize,
) -> usize {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => 0,
        checked_trees::CheckedBooleanExpression::Local { position } => {
            usize::from(*position == local)
        }
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_local_reference_count(operand, local)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right }
        | checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            checked_boolean_local_reference_count(left, local)
                .saturating_add(checked_boolean_local_reference_count(right, local))
        }
        checked_trees::CheckedBooleanExpression::Constant(_)
        | checked_trees::CheckedBooleanExpression::Parameter { .. }
        | checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => 0,
    }
}

pub(crate) fn is_structural_scalar_return_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
            is_structural_boolean_return_expression(expression, scalar_parameters, available_locals)
        }
        expression => is_branch_free_structural_integer_expression(
            expression,
            scalar_parameters,
            available_locals,
        ),
    }
}

pub(crate) fn is_structural_boolean_return_expression(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right }
        | checked_trees::CheckedBooleanExpression::And { left, right }
        | checked_trees::CheckedBooleanExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        checked_trees::CheckedBooleanExpression::ErasedParameter { .. } => false,
        checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        checked_trees::CheckedBooleanExpression::StructuralParameterField { path, .. } => {
            path.len() == 1
        }
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}

pub(crate) fn is_branch_free_structural_integer_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::StructuralParameterByteLength { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. }
        | CheckedScalarExpression::IntegerWrappingCast { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => false,
        CheckedScalarExpression::IntegerLiteral { .. } => true,
        CheckedScalarExpression::IntegerBinary { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        CheckedScalarExpression::IntegerBitwiseNot { operand, .. }
        | CheckedScalarExpression::IntegerWiden { operand, .. }
        | CheckedScalarExpression::IntegerExactCast { operand, .. } => {
            is_branch_free_structural_integer_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        CheckedScalarExpression::Parameter { position, .. } => *position < scalar_parameters,
        CheckedScalarExpression::ErasedParameter { .. } => false,
        CheckedScalarExpression::Local { position, .. } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        CheckedScalarExpression::IeeeFloatLiteral { .. }
        | CheckedScalarExpression::StructuralParameterField { .. }
        | CheckedScalarExpression::Boolean(_) => false,
    }
}

pub(crate) fn is_branch_free_structural_scalar_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::Boolean(expression) => {
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

pub(crate) fn is_branch_free_structural_boolean_expression(
    expression: &checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        checked_trees::CheckedBooleanExpression::Constant(_) => true,
        checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::IntegerComparison { left, right, .. } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        checked_trees::CheckedBooleanExpression::ErasedParameter { .. } => false,
        checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => false,
        checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        checked_trees::CheckedBooleanExpression::And { .. }
        | checked_trees::CheckedBooleanExpression::Or { .. } => false,
    }
}
