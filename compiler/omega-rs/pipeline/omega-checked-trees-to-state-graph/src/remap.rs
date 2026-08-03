use omega_state_graph::{
    Operation, OperationExpressionRefs, StateGraph, TransitionEdge, TransitionExpressionRefs,
};
use psi_arena::{Arena, HandleSpan};
use psi_checked_trees::expression::{ExpressionHandle, ExpressionTable};

pub(crate) fn append_remapped_operations(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    source_operations: &Arena<Operation>,
    operations: HandleSpan<Operation>,
) -> HandleSpan<Operation> {
    let mut remapped_operations = HandleSpan::empty();

    for operation in source_operations.span_or_empty(operations) {
        let operation = remap_operation(target, source_expressions, operation);
        target
            .operations
            .append_to_span(&mut remapped_operations, operation);
    }

    remapped_operations
}

pub(crate) fn append_remapped_transitions(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    source_transitions: &Arena<TransitionEdge>,
    transitions: HandleSpan<TransitionEdge>,
) -> HandleSpan<TransitionEdge> {
    let mut remapped_transitions = HandleSpan::empty();

    for transition in source_transitions.span_or_empty(transitions) {
        let transition = remap_transition(target, source_expressions, transition);
        target
            .transitions
            .append_to_span(&mut remapped_transitions, transition);
    }

    remapped_transitions
}

fn remap_operation(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    operation: &Operation,
) -> Operation {
    Operation {
        statement_index: operation.statement_index,
        kind: operation.kind.clone(),
        expressions: remap_operation_expression_refs(
            target,
            source_expressions,
            operation.expressions,
        ),
    }
}

fn remap_operation_expression_refs(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    expressions: OperationExpressionRefs,
) -> OperationExpressionRefs {
    match expressions {
        OperationExpressionRefs::Assignment { target: lhs, value } => {
            OperationExpressionRefs::Assignment {
                target: copy_expression(target, source_expressions, lhs),
                value: copy_expression(target, source_expressions, value),
            }
        }
        OperationExpressionRefs::Call { arguments } => OperationExpressionRefs::Call {
            arguments: copy_expression_span(target, source_expressions, arguments),
        },
        OperationExpressionRefs::Expression(expression) => OperationExpressionRefs::Expression(
            copy_expression(target, source_expressions, expression),
        ),
        OperationExpressionRefs::None => OperationExpressionRefs::None,
    }
}

fn remap_transition(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    transition: &TransitionEdge,
) -> TransitionEdge {
    TransitionEdge {
        statement_index: transition.statement_index,
        target: transition.target.clone(),
        continuation: transition.continuation.clone(),
        expressions: TransitionExpressionRefs {
            target_arguments: copy_expression_span(
                target,
                source_expressions,
                transition.expressions.target_arguments,
            ),
            target_value: transition
                .expressions
                .target_value
                .is_valid()
                .then(|| {
                    copy_expression(
                        target,
                        source_expressions,
                        transition.expressions.target_value,
                    )
                })
                .unwrap_or_else(ExpressionHandle::invalid),
            continuation_arguments: copy_expression_span(
                target,
                source_expressions,
                transition.expressions.continuation_arguments,
            ),
            continuation_value: transition
                .expressions
                .continuation_value
                .is_valid()
                .then(|| {
                    copy_expression(
                        target,
                        source_expressions,
                        transition.expressions.continuation_value,
                    )
                })
                .unwrap_or_else(ExpressionHandle::invalid),
            guard: transition
                .expressions
                .guard
                .is_valid()
                .then(|| copy_expression(target, source_expressions, transition.expressions.guard))
                .unwrap_or_else(ExpressionHandle::invalid),
        },
    }
}

fn copy_expression(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    expression: ExpressionHandle,
) -> ExpressionHandle {
    target.expressions.copy_from(source_expressions, expression)
}

fn copy_expression_span(
    target: &mut StateGraph,
    source_expressions: &ExpressionTable,
    expressions: HandleSpan<ExpressionHandle>,
) -> HandleSpan<ExpressionHandle> {
    target
        .expressions
        .copy_expression_handles_from(source_expressions, expressions)
}
