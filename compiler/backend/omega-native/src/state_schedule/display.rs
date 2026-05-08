use super::ScheduledState;
use super::lookups::{machine_flow_by_symbol, state_flow_by_key};
use crate::control_flow::StateKey;
use crate::plan::NativePlan;

pub(super) fn cycle_path(
    native_plan: &NativePlan,
    visited: &[ScheduledState],
    current: &ScheduledState,
) -> String {
    let start = visited
        .iter()
        .position(|state| state == current)
        .unwrap_or(0);
    visited[start..]
        .iter()
        .chain(std::iter::once(current))
        .map(|state| state_key_display(native_plan, state.key))
        .collect::<Vec<_>>()
        .join(" -> ")
}

pub(super) fn state_key_display(native_plan: &NativePlan, key: StateKey) -> String {
    let machine_name = machine_flow_by_symbol(native_plan, key.machine)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|_| format!("symbol{}", key.machine.arena_index()));
    let state_name = state_flow_by_key(native_plan, key)
        .map(|state| state.name.to_string())
        .unwrap_or_else(|_| format!("symbol{}", key.state.arena_index()));

    if key.segment_index == 0 {
        format!("{machine_name}.{state_name}")
    } else {
        format!("{machine_name}.{state_name}#{}", key.segment_index)
    }
}
