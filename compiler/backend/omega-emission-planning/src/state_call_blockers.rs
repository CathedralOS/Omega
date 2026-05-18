use crate::EmissionPlanningInput;
use omega_core::arena::Arena;
use omega_state_calls::StateCallLowering;
use omega_state_schedule::{ScheduledState, scheduled_state_contains_key};

use super::semantic_scope::{proof_scope_suffix, state_call_receiver_name, state_name};
use super::{EmissionBlocker, blocker};

mod runtime_body;

use runtime_body::collect_runtime_body_state_call_blockers;

pub(super) fn collect_state_call_blockers(
    input: &EmissionPlanningInput<'_>,
    state_schedule: &[ScheduledState],
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_state_call_blockers(input, blockers);
        collect_unresolved_state_call_blockers(input, blockers);
        return;
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        let source_name = state_name(input, state_call.source_key);
        if !state_call.target_key.is_valid() {
            blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} has unresolved state call through `{}`",
                    source_name,
                    state_call.statement_index,
                    state_call_receiver_name(input, state_call)
                ),
            ));
            continue;
        }
        let target_name = state_name(input, state_call.target_key);

        if matches!(
            state_call.lowering,
            StateCallLowering::InlineLeaf
                | StateCallLowering::InlineBranching
                | StateCallLowering::InlineExpansion
        ) && !needs_runtime_dispatch
            && scheduled_state_contains_key(state_schedule, state_call.source_key)
            && scheduled_state_contains_key(state_schedule, state_call.target_key)
        {
            continue;
        }

        match state_call.lowering {
            StateCallLowering::InlineLeaf => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls leaf state {} with {} argument(s){}; native emission needs leaf state-call inlining",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count,
                    proof_scope_suffix(input, state_call.source_key)
                ),
            )),
            StateCallLowering::InlineBranching => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls branching state {} with {} argument(s){}; native emission needs guarded state-call expansion",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count,
                    proof_scope_suffix(input, state_call.source_key)
                ),
            )),
            StateCallLowering::InlineExpansion => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls {} with {} argument(s){}; native emission needs inline state-call expansion",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count,
                    proof_scope_suffix(input, state_call.source_key)
                ),
            )),
            StateCallLowering::Unresolved => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} has unresolved state call through `{}`",
                    source_name,
                    state_call.statement_index,
                    state_call_receiver_name(input, state_call)
                ),
            )),
        };
    }
}

fn collect_unresolved_state_call_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required || state_call.target_key.is_valid() {
            continue;
        }

        let source_name = state_name(input, state_call.source_key);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "{} statement {} has unresolved state call through `{}`",
                source_name,
                state_call.statement_index,
                state_call_receiver_name(input, state_call)
            ),
        ));
    }
}
