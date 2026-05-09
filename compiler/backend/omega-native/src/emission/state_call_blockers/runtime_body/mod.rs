mod grouping;
mod model;
mod planned;
mod reasons;

use crate::plan::NativePlan;
use crate::runtime_dispatch::bodies::RuntimeDispatchBodyOperationKind;
use omega_core::arena::Arena;

use super::super::{EmissionBlocker, blocker};
use grouping::{push_runtime_body_state_call_blocker, repeated_count_suffix};
use model::RuntimeBodyStateCallBlocker;
use planned::runtime_body_state_call_has_planned_expansion;
use reasons::runtime_body_state_call_expansion_reason;

pub(super) fn collect_runtime_body_state_call_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let mut grouped_blockers = Vec::<RuntimeBodyStateCallBlocker>::new();

    for (_, body) in native_plan.runtime_bodies.bodies.iter() {
        let Some(operations) = native_plan
            .runtime_bodies
            .operations
            .paged_span(body.operations)
        else {
            blockers.insert(blocker(
                "runtime bodies",
                &format!(
                    "#{} {}.{} has an invalid runtime body operation span",
                    body.dispatch_index, body.machine, body.state
                ),
            ));
            continue;
        };

        for operation in operations.iter() {
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_key,
                target_machine,
                target_state,
                argument_count,
                lowering,
            } = &operation.kind
            else {
                continue;
            };

            let (source_machine, source_state) = state_names(native_plan, operation.source_key);
            push_runtime_body_state_call_blocker(
                &mut grouped_blockers,
                RuntimeBodyStateCallBlocker {
                    dispatch_index: body.dispatch_index,
                    source_key: operation.source_key,
                    source_machine,
                    source_state,
                    first_statement_index: operation.statement_index,
                    target_key: *target_key,
                    target_machine: target_machine.to_string(),
                    target_state: target_state.to_string(),
                    argument_count: *argument_count,
                    lowering: *lowering,
                    count: 1,
                },
            );
        }
    }

    for grouped_blocker in grouped_blockers {
        if runtime_body_state_call_has_planned_expansion(native_plan, &grouped_blocker) {
            continue;
        }

        let expansion_reason =
            runtime_body_state_call_expansion_reason(native_plan, &grouped_blocker);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "#{} {}.{} statement {} calls {}.{} with {} argument(s){}; runtime dispatch body needs {expansion_reason}",
                grouped_blocker.dispatch_index,
                grouped_blocker.source_machine,
                grouped_blocker.source_state,
                grouped_blocker.first_statement_index,
                grouped_blocker.target_machine,
                grouped_blocker.target_state,
                grouped_blocker.argument_count,
                repeated_count_suffix(grouped_blocker.count),
            ),
        ));
    }
}

fn state_names(native_plan: &NativePlan, key: crate::control_flow::StateKey) -> (String, String) {
    let machine = native_plan
        .control_flow
        .machine_by_symbol(key.machine)
        .map(|machine| machine.name.to_string())
        .unwrap_or_default();
    let state = native_plan
        .control_flow
        .state_by_key(key)
        .map(|state| state.name.to_string())
        .unwrap_or_default();
    (machine, state)
}
