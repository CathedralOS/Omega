use super::context::RuntimeDispatchBodyContext;
use omega_control_flow::{Operation, StateKey};
use omega_platform_interface::HostCall;
use omega_state_calls::StateCall;
use omega_state_storage::{StateLocalStorage, StateMutation};

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}

pub(super) fn state_operations(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> Option<&[Operation]> {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.key == state_key))
        .and_then(|state| context.control_flow.operations.span(state.operations))
}

pub(super) fn state_has_no_transitions(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
) -> bool {
    context
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_key.machine)
        .and_then(|(_, machine)| context.control_flow.states.span(machine.states))
        .and_then(|states| states.iter().find(|state| state.key == state_key))
        .and_then(|state| context.control_flow.transitions.span(state.transitions))
        .is_none_or(|transitions| transitions.is_empty())
}

pub(super) fn host_call_for_statement(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&HostCall> {
    context
        .host_calls
        .calls
        .iter()
        .find(|(_, host_call)| {
            state_key_matches_statement_source(host_call.source_key, state_key)
                && host_call.statement_index == statement_index
        })
        .map(|(_, host_call)| host_call)
}

pub(super) fn state_call_for_statement(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&StateCall> {
    context
        .state_calls
        .statement_call(state_key, statement_index)
}

pub(super) fn local_storage_for_statement(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&StateLocalStorage> {
    context
        .state_storage
        .locals
        .iter()
        .find(|(_, local)| {
            local.source_key == state_key && local.statement_index == statement_index
        })
        .map(|(_, local)| local)
}

pub(super) fn mutation_for_statement(
    context: &RuntimeDispatchBodyContext,
    state_key: StateKey,
    statement_index: usize,
) -> Option<&StateMutation> {
    context
        .state_storage
        .mutations
        .iter()
        .find(|(_, mutation)| {
            mutation.source_key == state_key && mutation.statement_index == statement_index
        })
        .map(|(_, mutation)| mutation)
}
