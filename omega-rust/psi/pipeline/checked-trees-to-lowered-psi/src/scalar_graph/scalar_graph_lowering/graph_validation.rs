//! Parameter type validation and short-circuit checks for scalar graphs.

use crate::emission::operation_emission::LoweredScalarBinding;
use crate::emission::operation_emission::boolean::LoweredBooleanReturnExpression;
use crate::emission::operation_emission::expressions::LoweredDirectExpression;
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarBranchTerminator;
use crate::scalar_graph::{LoweringError, ScalarType, unsupported};

pub(crate) fn validate_boolean_parameter_types(
    expression: &LoweredBooleanReturnExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. } => Ok(()),
        LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean type validation")
        }
        LoweredBooleanReturnExpression::Parameter { position }
        | LoweredBooleanReturnExpression::Local { position } => {
            if parameter_types.get(*position) == Some(&ScalarType::Boolean) {
                Ok(())
            } else {
                unsupported("scalar graph guard parameters must be Boolean")
            }
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_boolean_parameter_types(operand, parameter_types)
        }
        LoweredBooleanReturnExpression::Equal { left, right }
        | LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_boolean_parameter_types(left, parameter_types)?;
            validate_boolean_parameter_types(right, parameter_types)
        }
        LoweredBooleanReturnExpression::IntegerComparison { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
    }
}

pub(crate) fn validate_direct_parameter_types(
    expression: &LoweredDirectExpression,
    parameter_types: &[ScalarType],
) -> Result<(), LoweringError> {
    match expression {
        LoweredDirectExpression::Parameter {
            position,
            scalar_type,
        }
        | LoweredDirectExpression::Local {
            position,
            scalar_type,
        } => {
            if parameter_types.get(*position) == Some(scalar_type) {
                Ok(())
            } else {
                unsupported("scalar graph integer guard parameter type does not match")
            }
        }
        LoweredDirectExpression::IntegerLiteral { .. }
        | LoweredDirectExpression::PrimitiveRead { .. }
        | LoweredDirectExpression::StructuralField { .. }
        | LoweredDirectExpression::IeeeFloatLiteral { .. }
        | LoweredDirectExpression::ByteSequenceLength { .. }
        | LoweredDirectExpression::ByteSequenceFieldLength { .. } => Ok(()),
        LoweredDirectExpression::ByteSequenceRead { index, .. } => {
            validate_direct_parameter_types(index, parameter_types)
        }
        LoweredDirectExpression::IntegerBinary { left, right, .. } => {
            validate_direct_parameter_types(left, parameter_types)?;
            validate_direct_parameter_types(right, parameter_types)
        }
        LoweredDirectExpression::IntegerBitwiseNot { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerWiden { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::IntegerExactCast { operand, .. } => {
            validate_direct_parameter_types(operand, parameter_types)
        }
        LoweredDirectExpression::Boolean { expression } => {
            validate_boolean_parameter_types(expression, parameter_types)
        }
    }
}

pub(crate) fn validate_scalar_graph(
    state: usize,
    successors: &[Vec<usize>],
    visited: &mut [bool],
    active: &mut [bool],
    allow_cycle: bool,
) -> Result<(), LoweringError> {
    if active[state] {
        return if allow_cycle {
            Ok(())
        } else {
            unsupported("scalar graph control must be acyclic")
        };
    }
    if visited[state] {
        return Ok(());
    }
    active[state] = true;
    for successor in &successors[state] {
        validate_scalar_graph(*successor, successors, visited, active, allow_cycle)?;
    }
    active[state] = false;
    visited[state] = true;
    Ok(())
}

pub(crate) fn contains_short_circuit(expression: &LoweredBooleanReturnExpression) -> bool {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => false,
        LoweredBooleanReturnExpression::Not { operand } => contains_short_circuit(operand),
        LoweredBooleanReturnExpression::Equal { left, right } => {
            contains_short_circuit(left) || contains_short_circuit(right)
        }
        LoweredBooleanReturnExpression::And { .. } | LoweredBooleanReturnExpression::Or { .. } => {
            true
        }
    }
}

pub(crate) fn direct_expression_contains_short_circuit(
    expression: &LoweredDirectExpression,
) -> bool {
    matches!(
        expression,
        LoweredDirectExpression::Boolean { expression }
            if contains_short_circuit(expression)
    )
}

fn scalar_binding_contains_short_circuit(binding: &LoweredScalarBinding) -> bool {
    match binding {
        LoweredScalarBinding::SelectedComparison { left, right, .. } => {
            direct_expression_contains_short_circuit(left)
                || direct_expression_contains_short_circuit(right)
        }
        LoweredScalarBinding::Expression(expression) => {
            direct_expression_contains_short_circuit(expression)
        }
        LoweredScalarBinding::DirectCall(call) => call
            .arguments
            .iter()
            .any(direct_expression_contains_short_circuit),
        LoweredScalarBinding::StoredValue { value, .. } => {
            direct_expression_contains_short_circuit(value)
        }
    }
}

pub(crate) fn staged_short_circuit_bindings_terminator(
    bindings: &[LoweredScalarBinding],
    terminator: &LoweredScalarBranchTerminator,
) -> Option<(Vec<LoweredScalarBinding>, LoweredScalarBranchTerminator)> {
    if !bindings.iter().any(scalar_binding_contains_short_circuit) {
        return None;
    }
    Some((bindings.to_vec(), terminator.clone()))
}

pub(crate) fn validate_short_circuit_expression(
    expression: &LoweredBooleanReturnExpression,
) -> Result<(), LoweringError> {
    match expression {
        LoweredBooleanReturnExpression::Constant { .. }
        | LoweredBooleanReturnExpression::StructuralCaseMembership { .. }
        | LoweredBooleanReturnExpression::PrimitiveRead { .. }
        | LoweredBooleanReturnExpression::Parameter { .. }
        | LoweredBooleanReturnExpression::StructuralField { .. }
        | LoweredBooleanReturnExpression::Local { .. }
        | LoweredBooleanReturnExpression::IntegerComparison { .. } => Ok(()),
        LoweredBooleanReturnExpression::UnresolvedStructuralParameterField { .. } => {
            unsupported("unresolved structural field crossed Boolean validation")
        }
        LoweredBooleanReturnExpression::Not { operand } => {
            validate_short_circuit_expression(operand)
        }
        LoweredBooleanReturnExpression::Equal { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
        LoweredBooleanReturnExpression::And { left, right }
        | LoweredBooleanReturnExpression::Or { left, right } => {
            validate_short_circuit_expression(left)?;
            validate_short_circuit_expression(right)
        }
    }
}
