use crate::plan::NativePlan;
use crate::runtime_dispatch::loop_plan::{RuntimeDispatchLoopAction, RuntimeDispatchLoopEdge};
use crate::state_guards::{StateGuardLowering, StateGuardOperator};
use crate::state_schedule::{ScheduledState, scheduled_state_key};
use omega_core::arena::Arena;

use super::{EmissionBlocker, blocker};

pub(super) fn runtime_and_required_states(native_plan: &NativePlan) -> Vec<ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in native_plan.runtime_flow.states.iter() {
        push_scheduled_state(native_plan, &mut states, &state.machine, &state.state);
    }

    for (_, state_call) in native_plan.state_calls.calls.iter() {
        if state_call.required {
            push_scheduled_state_key(&mut states, state_call.source_key);

            if state_call.target_key.is_valid() {
                push_scheduled_state_key(&mut states, state_call.target_key);
            }
        }
    }

    states
}

fn push_scheduled_state(
    native_plan: &NativePlan,
    states: &mut Vec<ScheduledState>,
    machine: &str,
    state: &str,
) {
    let Some(key) = scheduled_state_key(native_plan, machine, state) else {
        return;
    };

    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(ScheduledState { key });
}

fn push_scheduled_state_key(states: &mut Vec<ScheduledState>, key: crate::control_flow::StateKey) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(ScheduledState { key });
}

pub(super) fn collect_runtime_dispatch_blockers(
    native_plan: &NativePlan,
    blockers: &mut Arena<EmissionBlocker>,
) {
    for (_, cycle) in native_plan.runtime_flow.cycles.iter() {
        let Some(states) = native_plan.runtime_flow.cycle_states.span(cycle.states) else {
            blockers.insert(blocker(
                "runtime dispatch",
                "invalid runtime cycle span in native flow plan",
            ));
            continue;
        };
        let cycle_path = states
            .iter()
            .map(|state| format!("{}.{}", state.machine, state.state))
            .collect::<Vec<_>>()
            .join(" -> ");

        blockers.insert(blocker(
            "runtime dispatch",
            &format!("cycle {cycle_path} needs generated state dispatch before native emission"),
        ));
    }
}

pub(super) fn runtime_dispatch_loop_blocker(native_plan: &NativePlan) -> EmissionBlocker {
    if let Some(guard_lowering) = first_unsupported_dispatch_guard(native_plan) {
        return blocker(
            "runtime dispatch",
            &format!(
                "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); guard lowering {guard_lowering:?} needs runtime state comparison byte emission",
                native_plan.runtime_dispatch_loop.cases.len(),
                native_plan.runtime_dispatch_loop.edges.len(),
                native_plan.runtime_flow.cycles.len()
            ),
        );
    }

    blocker(
        "runtime dispatch",
        &format!(
            "dispatch loop planned with {} case(s), {} edge(s), and {} cycle(s); native emission needs dispatch loop byte emission",
            native_plan.runtime_dispatch_loop.cases.len(),
            native_plan.runtime_dispatch_loop.edges.len(),
            native_plan.runtime_flow.cycles.len()
        ),
    )
}

pub(super) fn runtime_dispatch_loop_can_emit(native_plan: &NativePlan) -> bool {
    native_plan
        .runtime_dispatch_loop
        .edges
        .iter()
        .all(|(_, edge)| {
            dispatch_loop_guard_can_emit(edge) && edge.action != RuntimeDispatchLoopAction::Unknown
        })
}

fn first_unsupported_dispatch_guard(native_plan: &NativePlan) -> Option<StateGuardLowering> {
    native_plan
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
