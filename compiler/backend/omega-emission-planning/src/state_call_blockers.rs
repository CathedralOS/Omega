use omega_native::plan::NativePlan;
use omega_native::state_schedule::{ScheduledState, scheduled_state_contains_key};
use omega_control_flow::StateKey;
use omega_core::arena::Arena;
use omega_state_calls::StateCallLowering;

use super::{EmissionBlocker, blocker};

mod runtime_body;

use runtime_body::collect_runtime_body_state_call_blockers;

pub(super) fn collect_state_call_blockers(
    native_plan: &NativePlan,
    state_schedule: &[ScheduledState],
    needs_runtime_dispatch: bool,
    blockers: &mut Arena<EmissionBlocker>,
) {
    if needs_runtime_dispatch {
        collect_runtime_body_state_call_blockers(native_plan, blockers);
        collect_unresolved_state_call_blockers(native_plan, blockers);
        return;
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        let source_name = state_name(native_plan, state_call.source_key);
        if !state_call.target_key.is_valid() {
            blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} has unresolved state call through `{}`",
                    source_name, state_call.statement_index, state_call.receiver_display
                ),
            ));
            continue;
        }
        let target_name = state_name(native_plan, state_call.target_key);

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
                    "{} statement {} calls leaf state {} with {} argument(s); native emission needs leaf state-call inlining",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::InlineBranching => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls branching state {} with {} argument(s); native emission needs guarded state-call expansion",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::InlineExpansion => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} calls {} with {} argument(s); native emission needs inline state-call expansion",
                    source_name,
                    state_call.statement_index,
                    target_name,
                    state_call.argument_count
                ),
            )),
            StateCallLowering::Unresolved => blockers.insert(blocker(
                "state calls",
                &format!(
                    "{} statement {} has unresolved state call through `{}`",
                    source_name,
                    state_call.statement_index,
                    state_call.receiver_display
                ),
            )),
        };
    }
}

fn collect_unresolved_state_call_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if !state_call.required || state_call.target_key.is_valid() {
            continue;
        }

        let source_name = state_name(native_plan, state_call.source_key);
        blockers.insert(blocker(
            "state calls",
            &format!(
                "{} statement {} has unresolved state call through `{}`",
                source_name, state_call.statement_index, state_call.receiver_display
            ),
        ));
    }
}

fn state_name(native_plan: &NativePlan, key: StateKey) -> String {
    native_plan
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}
