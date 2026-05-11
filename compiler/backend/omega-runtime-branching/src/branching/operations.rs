use crate::RuntimeBranchingContext;
use omega_control_flow::{OperationKind, StateKey};
use omega_core::arena::{Arena, HandleSpan};
use omega_typed_trees::expression::ExpressionTable;

use super::lookups::{host_call_for_statement, mutation_for_statement, state_call_for_operation};
use super::{
    RuntimeBranchPreludeOperation, RuntimeBranchPreludeOperationKind,
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};

pub(super) fn prelude_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeBranchPreludeOperation>,
    source_key: StateKey,
) -> HandleSpan<RuntimeBranchPreludeOperation> {
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    output_operations.insert_many(operations.iter().map(|operation| {
        RuntimeBranchPreludeOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: prelude_operation_kind(
                context,
                expressions,
                source_key,
                operation.statement_index,
                &operation.kind,
            ),
        }
    }))
}

pub(super) fn leaf_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeLeafBranchOperation>,
    source_key: StateKey,
) -> HandleSpan<RuntimeLeafBranchOperation> {
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    output_operations.insert_many(
        operations
            .iter()
            .map(|operation| RuntimeLeafBranchOperation {
                source_key,
                statement_index: operation.statement_index,
                kind: leaf_operation_kind(
                    context,
                    expressions,
                    source_key,
                    operation.statement_index,
                ),
            }),
    )
}

pub(super) fn straight_line_operations(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    output_operations: &mut Arena<RuntimeStraightLineBranchOperation>,
    source_key: StateKey,
) -> HandleSpan<RuntimeStraightLineBranchOperation> {
    let Some(state) = context.control_flow.state_by_key(source_key) else {
        return HandleSpan::empty();
    };
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return HandleSpan::empty();
    };

    output_operations.insert_many(operations.iter().map(|operation| {
        RuntimeStraightLineBranchOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: straight_line_operation_kind(
                context,
                expressions,
                source_key,
                operation.statement_index,
                &operation.kind,
            ),
        }
    }))
}

fn prelude_operation_kind(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeBranchPreludeOperationKind {
    if let Some(host_call) = host_call_for_statement(context, source_key, statement_index) {
        return RuntimeBranchPreludeOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(context, source_key, statement_index) {
        return RuntimeBranchPreludeOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: expressions.copy_from(&context.state_storage.expressions, mutation.target),
            value: expressions.copy_from(&context.state_storage.expressions, mutation.value),
        };
    }

    if let Some(state_call) = state_call_for_operation(context, source_key, statement_index) {
        return RuntimeBranchPreludeOperationKind::StateCall {
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        };
    }

    if matches!(operation_kind, OperationKind::LocalData) {
        return RuntimeBranchPreludeOperationKind::LocalData;
    }

    RuntimeBranchPreludeOperationKind::Other
}

fn leaf_operation_kind(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
) -> RuntimeLeafBranchOperationKind {
    if let Some(host_call) = host_call_for_statement(context, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(context, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: expressions.copy_from(&context.state_storage.expressions, mutation.target),
            value: expressions.copy_from(&context.state_storage.expressions, mutation.value),
        };
    }

    RuntimeLeafBranchOperationKind::Other
}

fn straight_line_operation_kind(
    context: &RuntimeBranchingContext,
    expressions: &mut ExpressionTable,
    source_key: StateKey,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeStraightLineBranchOperationKind {
    if let Some(host_call) = host_call_for_statement(context, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(context, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: expressions.copy_from(&context.state_storage.expressions, mutation.target),
            value: expressions.copy_from(&context.state_storage.expressions, mutation.value),
        };
    }

    if let Some(state_call) = state_call_for_operation(context, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::StateCall {
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        };
    }

    if matches!(operation_kind, OperationKind::LocalData) {
        return RuntimeStraightLineBranchOperationKind::LocalData;
    }

    RuntimeStraightLineBranchOperationKind::Other
}
