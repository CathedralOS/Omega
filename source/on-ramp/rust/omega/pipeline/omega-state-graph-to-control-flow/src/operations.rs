use omega_control_flow::{Operation, OperationExpressionRefs, OperationKind};
use omega_state_graph::StateGraph;
use psi_arena::Arena;

use crate::arena_remap::remap_arena;
use crate::handles::remap_expression_span;

pub(crate) fn remap_operations(state_graph: &StateGraph) -> Arena<Operation> {
    remap_arena(&state_graph.operations, remap_operation_owned)
}

pub(crate) fn remap_operation_owned(operation: omega_state_graph::Operation) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: remap_operation_kind(operation.kind),
        expressions: remap_operation_expression_refs(operation.expressions),
    }
}

fn remap_operation_kind(kind: omega_state_graph::OperationKind) -> OperationKind {
    match kind {
        omega_state_graph::OperationKind::Assignment => OperationKind::Assignment,
        omega_state_graph::OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver,
            receiver,
            target,
        } => OperationKind::Call {
            receiver_symbol,
            target_symbol,
            has_receiver,
            receiver,
            target,
        },
        omega_state_graph::OperationKind::ConstantIntegerAssignment => {
            OperationKind::ConstantIntegerAssignment
        }
        omega_state_graph::OperationKind::Expression => OperationKind::Expression,
        omega_state_graph::OperationKind::LocalData => OperationKind::LocalData,
        omega_state_graph::OperationKind::StaticAssignment => OperationKind::StaticAssignment,
    }
}

fn remap_operation_expression_refs(
    expressions: omega_state_graph::OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        omega_state_graph::OperationExpressionRefs::Assignment { target, value } => {
            OperationExpressionRefs::Assignment { target, value }
        }
        omega_state_graph::OperationExpressionRefs::Call { arguments } => {
            OperationExpressionRefs::Call {
                arguments: remap_expression_span(arguments),
            }
        }
        omega_state_graph::OperationExpressionRefs::Expression(expression) => {
            OperationExpressionRefs::Expression(expression)
        }
        omega_state_graph::OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

#[cfg(test)]
mod tests;
