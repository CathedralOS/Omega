use crate::host_calls::HostCall;
use crate::plan::NativePlan;
use crate::state_calls::StateCall;
use omega_control_flow::{StateKey, StateParameterFlow};

pub(super) fn host_call_for_statement(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&HostCall> {
    native_plan
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn mutation_for_statement(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&crate::state_storage::StateMutation> {
    native_plan
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}

pub(super) fn state_call_for_operation(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&StateCall> {
    native_plan
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_key == source_key && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

pub(super) fn state_parameters(
    native_plan: &NativePlan,
    state_key: StateKey,
) -> Vec<StateParameterFlow> {
    native_plan
        .control_flow
        .state_by_key(state_key)
        .map(|state| state.parameters.to_vec())
        .unwrap_or_default()
}

pub(super) fn state_statement_has_host_call(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.source_key == source_key && host_call.statement_index == statement_index
    })
}
