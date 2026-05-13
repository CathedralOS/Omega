mod grouping;
mod model;
mod planned;
mod reasons;

use crate::EmissionPlanningInput;
use omega_core::arena::Arena;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;

use super::super::{EmissionBlocker, blocker};
use crate::semantic_scope::{proof_scope_suffix, state_name};
use grouping::{push_runtime_body_state_call_blocker, repeated_count_suffix};
use model::RuntimeBodyStateCallBlocker;
use planned::runtime_body_state_call_has_planned_expansion;
use reasons::runtime_body_state_call_expansion_reason;

pub(super) fn collect_runtime_body_state_call_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    let mut grouped_blockers = Vec::<RuntimeBodyStateCallBlocker>::new();

    for (_, body) in input.runtime_bodies.bodies.iter() {
        let Some(operations) = input.runtime_bodies.operations.paged_span(body.operations) else {
            let source_name = state_name(input, body.key);
            blockers.insert(blocker(
                "runtime bodies",
                &format!(
                    "#{} {} has an invalid runtime body operation span{}",
                    body.dispatch_index,
                    source_name,
                    proof_scope_suffix(input, body.key)
                ),
            ));
            continue;
        };

        for operation in operations.iter() {
            let RuntimeDispatchBodyOperationKind::StateCall {
                target_key,
                argument_count,
                lowering,
                ..
            } = &operation.kind
            else {
                continue;
            };

            let (source_machine, source_state) = state_names(input, operation.source_key);
            let (target_machine, target_state) = state_names(input, *target_key);
            push_runtime_body_state_call_blocker(
                &mut grouped_blockers,
                RuntimeBodyStateCallBlocker {
                    dispatch_index: body.dispatch_index,
                    source_key: operation.source_key,
                    source_machine,
                    source_state,
                    first_statement_index: operation.statement_index,
                    target_key: *target_key,
                    target_machine,
                    target_state,
                    argument_count: *argument_count,
                    lowering: *lowering,
                    count: 1,
                },
            );
        }
    }

    for grouped_blocker in grouped_blockers {
        if runtime_body_state_call_has_planned_expansion(input, &grouped_blocker) {
            continue;
        }

        let expansion_reason = runtime_body_state_call_expansion_reason(input, &grouped_blocker);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "#{} {}.{} statement {} calls {}.{} with {} argument(s){}; runtime dispatch body needs {expansion_reason}{}",
                grouped_blocker.dispatch_index,
                grouped_blocker.source_machine,
                grouped_blocker.source_state,
                grouped_blocker.first_statement_index,
                grouped_blocker.target_machine,
                grouped_blocker.target_state,
                grouped_blocker.argument_count,
                repeated_count_suffix(grouped_blocker.count),
                proof_scope_suffix(input, grouped_blocker.source_key)
            ),
        ));
    }
}

fn state_names(
    input: &EmissionPlanningInput<'_>,
    key: omega_control_flow::StateKey,
) -> (String, String) {
    state_name(input, key)
        .split_once('.')
        .map(|(machine, state)| (machine.to_owned(), state.to_owned()))
        .unwrap_or_default()
}
