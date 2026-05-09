use crate::RuntimeBranchingContext;
use omega_control_flow::{OperationKind, StateKey};

use super::lookups::{host_call_for_statement, mutation_for_statement, state_call_for_operation};
use super::{
    RuntimeLeafBranchOperation, RuntimeLeafBranchOperationKind, RuntimeStraightLineBranchOperation,
    RuntimeStraightLineBranchOperationKind,
};

pub(super) fn leaf_operations(
    native_plan: &RuntimeBranchingContext,
    source_key: StateKey,
) -> Vec<RuntimeLeafBranchOperation> {
    let Some(state) = native_plan.control_flow.state_by_key(source_key) else {
        return Vec::new();
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Vec::new();
    };

    operations
        .iter()
        .map(|operation| RuntimeLeafBranchOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: leaf_operation_kind(native_plan, source_key, operation.statement_index),
        })
        .collect()
}

pub(super) fn straight_line_operations(
    native_plan: &RuntimeBranchingContext,
    source_key: StateKey,
) -> Vec<RuntimeStraightLineBranchOperation> {
    let Some(state) = native_plan.control_flow.state_by_key(source_key) else {
        return Vec::new();
    };
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Vec::new();
    };

    operations
        .iter()
        .map(|operation| RuntimeStraightLineBranchOperation {
            source_key,
            statement_index: operation.statement_index,
            kind: straight_line_operation_kind(
                native_plan,
                source_key,
                operation.statement_index,
                &operation.kind,
            ),
        })
        .collect()
}

fn leaf_operation_kind(
    native_plan: &RuntimeBranchingContext,
    source_key: StateKey,
    statement_index: usize,
) -> RuntimeLeafBranchOperationKind {
    if let Some(host_call) = host_call_for_statement(native_plan, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(native_plan, source_key, statement_index) {
        return RuntimeLeafBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
        };
    }

    RuntimeLeafBranchOperationKind::Other
}

fn straight_line_operation_kind(
    native_plan: &RuntimeBranchingContext,
    source_key: StateKey,
    statement_index: usize,
    operation_kind: &OperationKind,
) -> RuntimeStraightLineBranchOperationKind {
    if let Some(host_call) = host_call_for_statement(native_plan, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::HostCall {
            platform_call: host_call.platform_call.clone(),
        };
    }

    if let Some(mutation) = mutation_for_statement(native_plan, source_key, statement_index) {
        return RuntimeStraightLineBranchOperationKind::Mutation {
            mutation_kind: mutation.mutation_kind,
            lowering: mutation.lowering,
            target: mutation.target.clone(),
            value: mutation.value.clone(),
        };
    }

    if let Some(state_call) = state_call_for_operation(native_plan, source_key, statement_index) {
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
