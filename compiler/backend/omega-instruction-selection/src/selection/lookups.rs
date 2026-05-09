use crate::InstructionSelectionInput;
use omega_control_flow::{Operation, StateKey, StateParameterFlow};
use omega_platform_interface::HostCall;
use omega_state_calls::StateCall;
use omega_state_storage::StateMutation;

pub(super) fn host_call_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan HostCall> {
    input
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            host_call.source_key == source_key && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn state_call_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateCall> {
    input
        .state_calls
        .calls
        .iter()
        .find(|(_, state_call)| {
            state_call.source_key == source_key && state_call.statement_index == statement_index
        })
        .map(|(_, state_call)| state_call)
}

pub(super) fn state_parameters(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
) -> Vec<StateParameterFlow> {
    input
        .control_flow
        .state_by_key(state_key)
        .map(|state| state.parameters.to_vec())
        .unwrap_or_default()
}

pub(super) fn state_operations<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    state_key: StateKey,
) -> Option<&'plan [Operation]> {
    input
        .control_flow
        .state_by_key(state_key)
        .and_then(|state| input.control_flow.operations.span(state.operations))
}

pub(super) fn state_mutation_for_statement<'plan>(
    input: &'plan InstructionSelectionInput<'plan>,
    source_key: StateKey,
    statement_index: usize,
) -> Option<&'plan StateMutation> {
    input
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}
