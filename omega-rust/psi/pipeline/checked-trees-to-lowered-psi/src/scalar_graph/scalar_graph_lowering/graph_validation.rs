//! Scalar graph cycle validation and short-circuit branch staging.

use crate::emission::expression_validation::direct_expression_contains_short_circuit;
use crate::emission::operation_emission::LoweredScalarBinding;
use crate::lowering_error::{LoweringError, unsupported};
use crate::scalar_graph::scalar_graph_lowering::prepared_graph::LoweredScalarBranchTerminator;

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
