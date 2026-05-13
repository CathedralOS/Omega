use crate::RuntimeBranchingContext;
use omega_control_flow::{StateKey, StateParameterFlow};
use omega_platform_interface::HostCall;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::{StateCall, StateCallRole};

pub(super) fn host_call_for_statement<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    context
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn mutation_for_statement<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan omega_state_storage::StateMutation> {
    context
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

pub(super) fn state_call_for_operation<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    context.state_calls.statement_call(source_key, statement_index)
}

pub(super) fn state_call_for_runtime_operation<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    operation: &RuntimeDispatchBodyOperation,
) -> Option<&'plan StateCall> {
    let role = match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { role, .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { role, .. }
        | RuntimeDispatchBodyOperationKind::StateCall { role, .. } => *role,
        _ => StateCallRole::Statement,
    };

    context
        .state_calls
        .call_for_role(operation.source_key, operation.statement_index, role)
}

pub(super) fn state_parameters<'plan>(
    context: &RuntimeBranchingContext<'plan>,
    state_key: StateKey,
) -> &'plan [StateParameterFlow] {
    context
        .control_flow
        .state_by_key(state_key)
        .map(|state| state.parameters.as_slice())
        .unwrap_or(&[])
}

pub(super) fn state_statement_has_host_call(
    context: &RuntimeBranchingContext,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    context.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.source_key == source_key && host_call.statement_index == statement_index
    })
}
