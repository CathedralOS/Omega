use crate::control_flow::{OperationKind, StateFlow, StateKey};
use crate::plan::NativePlan;
use crate::state_schedule::{ScheduledState, scheduled_state_flow};
use omega_core::arena::Arena;

use super::{EmissionBlocker, blocker};

pub(super) fn collect_state_codegen_blockers(
    native_plan: &NativePlan,
    state_schedule: &[ScheduledState],
    blockers: &mut Arena<EmissionBlocker>,
) {
    for scheduled_state in state_schedule {
        let Some(state_flow) = scheduled_state_flow(native_plan, scheduled_state) else {
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
        let machine_name =
            machine_name_for_state(native_plan, state_flow).unwrap_or("<missing-machine>");
        let state_name = state_flow.name.as_str();

        let Some(operations) = native_plan
            .control_flow
            .operations
            .span(state_flow.operations)
        else {
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
                        native_plan,
                        state_flow.key,
                        operation.statement_index,
                    ) || state_statement_has_state_call(
                        native_plan,
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
                OperationKind::ConstantIntegerAssignment
                | OperationKind::StaticAssignment { .. } => {}
                OperationKind::Assignment { .. }
                    if state_statement_has_storage_mutation(
                        native_plan,
                        state_flow.key,
                        operation.statement_index,
                    ) => {}
                OperationKind::Assignment { .. } => {
                    blockers.insert(blocker(
                        "state codegen",
                        &format!(
                            "{machine_name}.{state_name} statement {} Assignment is not supported by native emission yet",
                            operation.statement_index
                        ),
                    ));
                }
                OperationKind::LocalData
                    if state_statement_has_local_storage(
                        native_plan,
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

fn state_statement_has_local_storage(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.state_storage.locals.iter().any(|(_, local)| {
        local.source_key == source_key && local.statement_index == statement_index
    })
}

fn state_statement_has_storage_mutation(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan
        .state_storage
        .mutations
        .iter()
        .any(|(_, mutation)| {
            mutation.source_key == source_key && mutation.statement_index == statement_index
        })
}

fn state_statement_has_state_call(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.state_calls.calls.iter().any(|(_, state_call)| {
        state_call.source_key == source_key && state_call.statement_index == statement_index
    })
}

fn machine_name_for_state<'plan>(
    native_plan: &'plan NativePlan,
    state_flow: &StateFlow,
) -> Option<&'plan str> {
    native_plan
        .control_flow
        .machines
        .iter()
        .find(|(_, machine)| machine.symbol == state_flow.key.machine)
        .map(|(_, machine)| machine.name.as_str())
}

fn state_statement_has_host_call(
    native_plan: &NativePlan,
    source_key: StateKey,
    statement_index: usize,
) -> bool {
    native_plan.host_calls.calls.iter().any(|(_, host_call)| {
        host_call.source_key == source_key && host_call.statement_index == statement_index
    })
}
