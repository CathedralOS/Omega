use crate::EmissionPlanningInput;
use omega_core::arena::Arena;
use omega_runtime_dispatch_loop::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use omega_state_guards::{StateGuardLowering, StateGuardOperator};
use omega_state_schedule::ScheduledState;

use super::{EmissionBlocker, blocker};

pub(super) fn runtime_and_required_states(
    input: &EmissionPlanningInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in input.runtime_flow.states.iter() {
        push_scheduled_state_key(&mut states, state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if state_call.required {
            push_scheduled_state_key(&mut states, state_call.source_key);

            if state_call.target_key.is_valid() {
                push_scheduled_state_key(&mut states, state_call.target_key);
            }
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

fn state_name(input: &EmissionPlanningInput<'_>, key: omega_control_flow::StateKey) -> String {
    input
        .control_flow
        .state_names_by_key(key)
        .map(|(machine, state)| format!("{machine}.{state}"))
        .unwrap_or_else(|| "<unknown>.<unknown>".to_owned())
}

pub(super) fn collect_runtime_dispatch_blockers(
    input: &EmissionPlanningInput<'_>,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, cycle) in input.runtime_flow.cycles.iter() {
        let Some(states) = input.runtime_flow.cycle_states.span(cycle.states) else {
            blockers.insert(blocker(
                "runtime dispatch",
                "invalid runtime cycle span in native flow plan",
            ));
            continue;
        };
        let cycle_path = states
            .iter()
            .map(|state| state_name(input, state.key))
            .collect::<Vec<_>>()
            .join(" -> ");

        blockers.insert(blocker(
            "runtime dispatch",
            &format!("cycle {cycle_path} needs generated state dispatch before native emission"),
        ));
    }
}

pub(super) fn runtime_dispatch_loop_blocker(input: &EmissionPlanningInput<'_>) -> EmissionBlocker {
    if let Some(guard_lowering) = first_unsupported_dispatch_guard(input) {
        return blocker(
            "runtime dispatch",
            &format!(
                "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); guard lowering {guard_lowering:?} needs runtime state comparison byte emission",
                input.runtime_dispatch_loop.cases.len(),
                input.runtime_dispatch_loop.edges.len(),
                input.runtime_flow.cycles.len()
            ),
        );
    }

    blocker(
        "runtime dispatch",
        &format!(
            "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); native emission needs dispatch loop byte emission",
            input.runtime_dispatch_loop.cases.len(),
            input.runtime_dispatch_loop.edges.len(),
            input.runtime_flow.cycles.len()
        ),
    )
}

pub(super) fn runtime_dispatch_loop_can_emit(input: &EmissionPlanningInput<'_>) -> bool {
    input.runtime_dispatch_loop.edges.iter().all(|(_, edge)| {
        dispatch_loop_guard_can_emit(edge) && edge.action != RuntimeDispatchLoopAction::Unknown
    })
}

fn first_unsupported_dispatch_guard(
    input: &EmissionPlanningInput<'_>,
) -> Option<StateGuardLowering> {
    input
        .runtime_dispatch_loop
        .edges
        .iter()
        .find(|(_, edge)| !dispatch_loop_guard_can_emit(edge))
        .map(|(_, edge)| edge.guard_lowering)
}

fn dispatch_loop_guard_can_emit(edge: &RuntimeDispatchLoopEdge) -> bool {
    match edge.guard_lowering {
        StateGuardLowering::NoOp => true,
        StateGuardLowering::CompareStaticValue => {
            edge.guard_has_storage
                && matches!(
                    edge.guard_operator,
                    StateGuardOperator::Equal | StateGuardOperator::NotEqual
                )
                && matches!(edge.guard_byte_size, 1 | 4)
        }
        StateGuardLowering::CompareRuntimeValue | StateGuardLowering::NeedsRuntimeExpression => {
            false
        }
    }
}
