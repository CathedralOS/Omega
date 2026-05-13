use super::context::RuntimeDispatchBodyContext;
use super::lookups::{
    host_call_for_statement, local_storage_for_statement, mutation_for_statement,
    state_call_for_statement, state_has_no_transitions, state_operations,
};
use super::model::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_control_flow::{OperationKind, StateKey};
use omega_state_calls::{StateCall, StateCallLowering};
use omega_state_dispatch::DispatchState;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchBody {
    pub key: StateKey,
    pub dispatch_index: u32,
    pub operations: Vec<RuntimeDispatchBodyOperation>,
}

pub(super) fn build_dispatch_body(
    context: &RuntimeDispatchBodyContext,
    dispatch_state: &DispatchState,
) -> CollectedRuntimeDispatchBody {
    let mut operations = Vec::new();
    append_state_body_operations(
        context,
        dispatch_state.key,
        &mut operations,
        &mut Vec::new(),
    );

    CollectedRuntimeDispatchBody {
        key: dispatch_state.key,
        dispatch_index: dispatch_state.dispatch_index,
        operations,
    }
}

fn append_state_body_operations(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    visiting: &mut Vec<StateKey>,
) {
    if visiting.contains(&state_key) {
        return;
    }
    visiting.push(state_key);

    let Some(state_operations) = state_operations(context, state_key) else {
        visiting.pop();
        return;
    };

    for operation in state_operations {
        if let Some(host_call) =
            host_call_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::HostCall {
                    platform_call: host_call.platform_call.clone(),
                },
            ));
            continue;
        }

        if let Some(state_call) =
            state_call_for_statement(context, state_key, operation.statement_index)
        {
            append_state_call_body_operation(context, state_call, operations, visiting);
            continue;
        }

        if let Some(local_storage) =
            local_storage_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::LocalStorage {
                    symbol: local_storage.symbol,
                    name: local_storage.name.clone(),
                    type_symbol: local_storage.type_symbol,
                    type_name: local_storage.type_name.clone(),
                    invariant_names: local_storage.invariant_names.clone(),
                },
            ));
            continue;
        }

        if let Some(mutation) =
            mutation_for_statement(context, state_key, operation.statement_index)
        {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Mutation {
                    mutation_kind: mutation.mutation_kind,
                    lowering: mutation.lowering,
                },
            ));
            continue;
        }

        if !matches!(operation.kind, OperationKind::LocalData) {
            operations.push(body_operation(
                state_key,
                operation.statement_index,
                RuntimeDispatchBodyOperationKind::Other,
            ));
        }
    }

    visiting.pop();
}

fn append_state_call_body_operation(
    context: &RuntimeDispatchBodyContext,
    state_call: &StateCall,
    operations: &mut Vec<RuntimeDispatchBodyOperation>,
    visiting: &mut Vec<StateKey>,
) {
    if state_call.lowering == StateCallLowering::InlineLeaf {
        operations.push(body_operation(
            state_call.source_key,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
                target_key: state_call.target_key,
                argument_count: state_call.argument_count,
            },
        ));
        append_state_body_operations(context, state_call.target_key, operations, visiting);
        return;
    }

    if state_has_no_transitions(context, state_call.target_key) {
        operations.push(body_operation(
            state_call.source_key,
            state_call.statement_index,
            RuntimeDispatchBodyOperationKind::InlineStateCall {
                target_key: state_call.target_key,
                argument_count: state_call.argument_count,
                lowering: state_call.lowering,
            },
        ));
        append_state_body_operations(context, state_call.target_key, operations, visiting);
        return;
    }

    operations.push(body_operation(
        state_call.source_key,
        state_call.statement_index,
        RuntimeDispatchBodyOperationKind::StateCall {
            target_key: state_call.target_key,
            argument_count: state_call.argument_count,
            lowering: state_call.lowering,
        },
    ));
}

fn body_operation(
    source_key: StateKey,
    statement_index: usize,
    kind: RuntimeDispatchBodyOperationKind,
) -> RuntimeDispatchBodyOperation {
    RuntimeDispatchBodyOperation {
        source_key,
        statement_index,
        kind,
    }
}
