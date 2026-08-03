mod grouping;
mod model;
mod planned;
mod reasons;

use crate::EmissionPlanningInput;
use omega_runtime_bodies::RuntimeDispatchBodyOperationKind;
use psi_arena::Arena;

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

            push_runtime_body_state_call_blocker(
                &mut grouped_blockers,
                RuntimeBodyStateCallBlocker {
                    dispatch_index: body.dispatch_index,
                    source_key: operation.source_key,
                    first_statement_index: operation.statement_index,
                    target_key: *target_key,
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
        let source_name = state_name(input, grouped_blocker.source_key);
        let target_name = state_name(input, grouped_blocker.target_key);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "#{} {} statement {} calls {} with {} argument(s){}; runtime dispatch body needs {expansion_reason}{}",
                grouped_blocker.dispatch_index,
                source_name,
                grouped_blocker.first_statement_index,
                target_name,
                grouped_blocker.argument_count,
                repeated_count_suffix(grouped_blocker.count),
                proof_scope_suffix(input, grouped_blocker.source_key)
            ),
        ));
    }
}
