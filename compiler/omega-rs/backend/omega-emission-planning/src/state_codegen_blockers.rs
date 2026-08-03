use crate::EmissionPlanningInput;
use omega_control_flow::{OperationKind, StateFlow, StateKey};
use omega_state_schedule::{ScheduledState, StateScheduleContext, scheduled_state_flow};
use psi_arena::Arena;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_codegen_blockers(
    input: &EmissionPlanningInput<'_>,
    schedule_context: &StateScheduleContext,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for scheduled_state in state_schedule {
        let Some(state_flow) = scheduled_state_flow(schedule_context, scheduled_state) else {
            blockers.insert(blocker(
                "state codegen",
                &format!(
                    "scheduled state {}.{}#{} was not present in the control-flow plan",
                    scheduled_state.key.machine.arena_index(),
                    scheduled_state.key.state.arena_index(),
                    scheduled_state.key.segment_index
                ),
            ));
            continue;
        };
        let machine_name = machine_name_for_state(input, state_flow).unwrap_or("<missing-machine>");
        let state_name = state_flow.name.as_str();

        let Some(operations) = input.control_flow.operations.span(state_flow.operations) else {
            blockers.insert(blocker(
                "state codegen",
                &format!("{machine_name}.{state_name} has an invalid operation span"),
            ));
            continue;
        };

        for operation in operations {
            match operation.kind {
                OperationKind::Call { .. }
                    if state_statement_has_host_call(
                        input,
                        state_flow.key,
                        operation.statement_index,
                    ) || state_statement_has_state_call(
                        input,
                        state_flow.key,
                        operation.statement_index,
                    ) || crate::state_call_blockers::statement_has_wire_encode_lowering(
                        input,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                OperationKind::Call { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{machine_name}.{state_name} statement {} is a call that is not lowered to a native host operation",
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::ConstantIntegerAssignment | OperationKind::StaticAssignment => {}
                OperationKind::Assignment
                    if state_statement_has_storage_mutation(
                        input,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                OperationKind::Assignment => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{machine_name}.{state_name} statement {} Assignment is not supported by native emission yet",
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::LocalData => {}
                OperationKind::Expression
                    if state_statement_has_expression_lowering(
                        input,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                _ => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{machine_name}.{state_name} statement {} {:?} is not supported by native emission yet",
                            operation.statement_index, operation.kind
                        ),
                    ));
                }
            };
        }
    }
}

fn state_statement_has_storage_mutation(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input.state_storage.mutations.iter().any(|(_, mutation)| {
        state_key_matches_statement_source(mutation.source_key, source_key)
            && mutation.statement_index == statement_index
    })
}

fn state_statement_has_expression_lowering(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    state_statement_has_storage_mutation(input, source_key, statement_index)
        || state_statement_has_host_call(input, source_key, statement_index)
        || state_statement_has_state_call(input, source_key, statement_index)
        || input.state_values.values.iter().any(|(_, value)| {
            value.source_key == source_key && value.statement_index == statement_index
        })
}

fn state_statement_has_state_call(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input
        .state_calls
        .statement_call(source_key, statement_index)
        .is_some()
}

fn machine_name_for_state<'plan>(
    input: &'plan EmissionPlanningInput<'plan>,
    state_flow: &StateFlow,
) -> Option<&'plan str> {
    input
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_flow.key.machine)
        .map(|(_, machine)| machine.name.as_str())
}

fn state_statement_has_host_call(
    input: &EmissionPlanningInput<'_>,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    input.host_calls.calls.iter().any(|(_, host_call)| {
        state_key_matches_statement_source(host_call.source_key, source_key)
            && host_call.statement_index == statement_index
    })
}

fn state_key_matches_statement_source(expected: StateKey, actual: StateKey) -> bool {
    expected == actual || (expected.machine == actual.machine && expected.state == actual.state)
}
