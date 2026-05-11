use crate::InstructionSelectionInput;
use omega_control_flow::{OperationKind, PlannedTransitionTarget, StateKey};
use omega_core::arena::Arena;
use omega_state_schedule::ScheduledState;

use super::host_operations::select_host_call;
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::{host_call_for_statement, state_call_for_statement};
use omega_target_operations::InstructionOperand;

pub(super) fn select_state_body_instructions(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut Vec<StateKey>,
) {
    if visiting.contains(&state_key) {
        return;
    }

    visiting.push(state_key);

    let Some(state) = input.control_flow.state_by_key(state_key) else {
        visiting.pop();
        return;
    };
    let Some(operations) = input.control_flow.operations.span(state.operations) else {
        visiting.pop();
        return;
    };
    let transitions = input.control_flow.transitions.span_or_empty(state.transitions);

    for operation in operations {
        if let Some(host_call) =
            host_call_for_statement(input, state.key, operation.statement_index)
        {
            select_host_call(input, host_call, operands, selected_instructions);
            continue;
        }

        let OperationKind::Call { .. } = &operation.kind else {
            continue;
        };
        let Some(state_call) =
            state_call_for_statement(input, state.key, operation.statement_index)
        else {
            continue;
        };

        if !state_call.target_key.is_valid() {
            continue;
        }

        select_state_body_instructions(
            input,
            state_call.target_key,
            operands,
            selected_instructions,
            visiting,
        );
    }

    for transition in transitions {
        follow_transition_target(
            input,
            &transition.target,
            operands,
            selected_instructions,
            visiting,
        );
        if let Some(continuation) = &transition.continuation {
            follow_transition_target(
                input,
                continuation,
                operands,
                selected_instructions,
                visiting,
            );
        }
    }

    visiting.pop();
}

fn follow_transition_target(
    input: &InstructionSelectionInput<'_>,
    target: &PlannedTransitionTarget,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut Vec<StateKey>,
) {
    let PlannedTransitionTarget::State { key, .. } = target else {
        return;
    };

    if !key.is_valid() {
        return;
    }

    select_state_body_instructions(input, *key, operands, selected_instructions, visiting);
}

pub(super) fn runtime_reachable_states(
    input: &InstructionSelectionInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in input.runtime_flow.states.iter() {
        push_scheduled_state_key(&mut states, state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        push_scheduled_state_key(&mut states, state_call.source_key);

        if state_call.target_key.is_valid() {
            push_scheduled_state_key(&mut states, state_call.target_key);
        }
    }

    states
}

fn push_scheduled_state_key(states: &mut Vec<ScheduledState>, key: omega_control_flow::StateKey) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(ScheduledState { key });
}
