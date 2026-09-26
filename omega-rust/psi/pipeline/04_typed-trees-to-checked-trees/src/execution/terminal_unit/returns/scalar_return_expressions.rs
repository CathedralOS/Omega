//! Classification of scalar and boolean return expressions.

use crate::execution::terminal_unit::CheckedScalarExpression;

pub(crate) fn checked_boolean_contains_short_circuit(
    expression: &crate::checked_trees::CheckedBooleanExpression,
) -> bool {
    match expression {
        crate::checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::And { .. }
        | crate::checked_trees::CheckedBooleanExpression::Or { .. } => true,
        crate::checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_contains_short_circuit(operand)
        }
        crate::checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            checked_boolean_contains_short_circuit(left)
                || checked_boolean_contains_short_circuit(right)
        }
        crate::checked_trees::CheckedBooleanExpression::Constant(_)
        | crate::checked_trees::CheckedBooleanExpression::Parameter { .. }
        | crate::checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | crate::checked_trees::CheckedBooleanExpression::Local { .. }
        | crate::checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | crate::checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
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
    expression: &crate::checked_trees::CheckedBooleanExpression,
    local: usize,
) -> usize {
    match expression {
        crate::checked_trees::CheckedBooleanExpression::StorageRead { .. } => 0,
        crate::checked_trees::CheckedBooleanExpression::Local { position } => {
            usize::from(*position == local)
        }
        crate::checked_trees::CheckedBooleanExpression::Not(operand) => {
            checked_boolean_local_reference_count(operand, local)
        }
        crate::checked_trees::CheckedBooleanExpression::Equal { left, right }
        | crate::checked_trees::CheckedBooleanExpression::And { left, right }
        | crate::checked_trees::CheckedBooleanExpression::Or { left, right } => {
            checked_boolean_local_reference_count(left, local)
                .saturating_add(checked_boolean_local_reference_count(right, local))
        }
        crate::checked_trees::CheckedBooleanExpression::Constant(_)
        | crate::checked_trees::CheckedBooleanExpression::Parameter { .. }
        | crate::checked_trees::CheckedBooleanExpression::ErasedParameter { .. }
        | crate::checked_trees::CheckedBooleanExpression::StructuralParameterField { .. }
        | crate::checked_trees::CheckedBooleanExpression::IntegerComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => 0,
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
    expression: &crate::checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        crate::checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::Constant(_) => true,
        crate::checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_structural_boolean_return_expression(operand, scalar_parameters, available_locals)
        }
        crate::checked_trees::CheckedBooleanExpression::Equal { left, right }
        | crate::checked_trees::CheckedBooleanExpression::And { left, right }
        | crate::checked_trees::CheckedBooleanExpression::Or { left, right } => {
            is_structural_boolean_return_expression(left, scalar_parameters, available_locals)
                && is_structural_boolean_return_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        crate::checked_trees::CheckedBooleanExpression::IntegerComparison {
            left, right, ..
        }
        | crate::checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison {
            left,
            right,
            ..
        } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        crate::checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        crate::checked_trees::CheckedBooleanExpression::ErasedParameter { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        crate::checked_trees::CheckedBooleanExpression::StructuralParameterField {
            path, ..
        } => path.len() == 1,
        crate::checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
    }
}

pub(crate) fn is_branch_free_structural_integer_expression(
    expression: &CheckedScalarExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        CheckedScalarExpression::StorageRead { .. }
        | CheckedScalarExpression::IntegerTrappingCast { .. }
        | CheckedScalarExpression::IntegerWrappingCast { .. }
        | CheckedScalarExpression::StructuralParameterIndexedRead { .. } => false,
        // A whole-view `source.len` observes the exact borrowed view the
        // structural parameter itself carries; a field-path byte length is a
        // nested view projection outside this lane.
        CheckedScalarExpression::StructuralParameterByteLength { path, .. } => path.is_empty(),
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
        | CheckedScalarExpression::IntegerExactCast { operand, .. }
        | CheckedScalarExpression::IntegerSaturatingCast { operand, .. } => {
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
    expression: &crate::checked_trees::CheckedBooleanExpression,
    scalar_parameters: usize,
    available_locals: usize,
) -> bool {
    match expression {
        crate::checked_trees::CheckedBooleanExpression::StorageRead { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::Constant(_) => true,
        crate::checked_trees::CheckedBooleanExpression::Not(operand) => {
            is_branch_free_structural_boolean_expression(
                operand,
                scalar_parameters,
                available_locals,
            )
        }
        crate::checked_trees::CheckedBooleanExpression::Equal { left, right } => {
            is_branch_free_structural_boolean_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_boolean_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        crate::checked_trees::CheckedBooleanExpression::IntegerComparison {
            left, right, ..
        }
        | crate::checked_trees::CheckedBooleanExpression::ScalarIeeeFloatComparison {
            left,
            right,
            ..
        } => {
            is_branch_free_structural_integer_expression(left, scalar_parameters, available_locals)
                && is_branch_free_structural_integer_expression(
                    right,
                    scalar_parameters,
                    available_locals,
                )
        }
        crate::checked_trees::CheckedBooleanExpression::Parameter { position } => {
            *position < scalar_parameters
        }
        crate::checked_trees::CheckedBooleanExpression::ErasedParameter { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::Local { position } => {
            *position >= scalar_parameters
                && *position < scalar_parameters.saturating_add(available_locals)
        }
        crate::checked_trees::CheckedBooleanExpression::StructuralParameterField { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::IeeeFloatComparison { .. }
        | crate::checked_trees::CheckedBooleanExpression::ByteSequenceEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::PayloadlessSumEqual { .. }
        | crate::checked_trees::CheckedBooleanExpression::StructuralCaseMembership { .. } => false,
        crate::checked_trees::CheckedBooleanExpression::And { .. }
        | crate::checked_trees::CheckedBooleanExpression::Or { .. } => false,
    }
}
