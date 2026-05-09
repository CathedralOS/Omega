use super::context::RuntimeDispatchBodyContext;
use super::lookups::{
    host_call_for_statement, local_storage_for_statement, mutation_for_statement,
    state_call_for_statement, state_has_no_transitions, state_operations,
};
use super::model::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use crate::control_flow::{OperationKind, StateKey};
use crate::runtime_dispatch::states::DispatchState;
use crate::state_calls::{StateCall, StateCallLowering};
use omega_typed_program::name::ProgramName;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CollectedRuntimeDispatchBody {
    pub key: StateKey,
    pub machine: ProgramName,
    pub state: ProgramName,
    pub dispatch_index: u32,
    pub operations: Vec<RuntimeDispatchBodyOperation>,
}

pub(super) fn build_dispatch_body(
    context: &RuntimeDispatchBodyContext,
    dispatch_state: &DispatchState,
) -> CollectedRuntimeDispatchBody {
    let (machine_name, state_name) = state_names(context, dispatch_state.key);
    let mut operations = Vec::new();
    append_state_body_operations(
        context,
        dispatch_state.key,
        &mut operations,
        &mut Vec::new(),
    );

    CollectedRuntimeDispatchBody {
        key: dispatch_state.key,
        machine: machine_name,
        state: state_name,
        dispatch_index: dispatch_state.dispatch_index,
        operations,
    }
}

fn state_names(context: &RuntimeDispatchBodyContext, key: StateKey) -> (ProgramName, ProgramName) {
    context
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| (machine.clone(), state.clone()))
        .unwrap_or_default()
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
                    name: local_storage.name.clone(),
                    type_name: local_storage.type_name.clone(),
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
                target_machine: state_call.target_machine.clone(),
                target_state: state_call.target_state.clone(),
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
                target_machine: state_call.target_machine.clone(),
                target_state: state_call.target_state.clone(),
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
            target_machine: state_call.target_machine.clone(),
            target_state: state_call.target_state.clone(),
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
