use super::ScheduledState;
use super::lookups::{machine_flow_by_symbol, state_flow_by_key};
use crate::StateScheduleContext;
use omega_control_flow::StateKey;

pub(super) fn cycle_path(
    context: &StateScheduleContext,
    visited: impl Iterator<Item = ScheduledState>,
    current: &ScheduledState,
) -> String {
    let mut inside_cycle = false;
    let mut path = visited
        .filter_map(|state| {
            if state == *current {
                inside_cycle = true;
            }

            inside_cycle.then(|| state_key_display(context, state.key))
        })
        .collect::<Vec<_>>();
    path.push(state_key_display(context, current.key));
    path.join(" -> ")
}

pub(super) fn state_key_display(context: &StateScheduleContext, key: StateKey) -> String {
    let machine_name = machine_flow_by_symbol(context, key.machine)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|_| format!("symbol{}", key.machine.arena_index()));
    let state_name = state_flow_by_key(context, key)
        .map(|state| state.name.to_string())
        .unwrap_or_else(|_| format!("symbol{}", key.state.arena_index()));

    if key.segment_index == 0 {
        format!("{machine_name}.{state_name}")
    } else {
        format!("{machine_name}.{state_name}#{}", key.segment_index)
    }
}
