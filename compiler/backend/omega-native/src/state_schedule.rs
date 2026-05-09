use crate::plan::NativePlan;
use omega_control_flow::{StateFlow, StateKey};

mod display;
mod local_calls;
mod lookups;
mod model;
mod static_values;
mod transitions;

use display::{cycle_path, state_key_display};
use local_calls::append_local_state_calls;
pub use model::ScheduledState;
use static_values::{
    PlaceKey, apply_static_operations, select_transition as select_static_transition,
};
use transitions::next_state;

pub fn build_entry_state_schedule(native_plan: &NativePlan) -> Result<Vec<ScheduledState>, String> {
    let mut schedule = Vec::new();
    let mut visited = Vec::<ScheduledState>::new();
    let mut values = Vec::<(PlaceKey, String)>::new();
    let mut aliases = Vec::<(PlaceKey, PlaceKey)>::new();

    append_state_chain(
        native_plan,
        native_plan.entry_key,
        &mut schedule,
        &mut visited,
        &mut values,
        &mut aliases,
    )?;

    Ok(schedule)
}

pub fn scheduled_state_contains_key(schedule: &[ScheduledState], state_key: StateKey) -> bool {
    schedule.iter().any(|scheduled| scheduled.key == state_key)
}

pub fn scheduled_state_flow<'plan>(
    native_plan: &'plan NativePlan,
    scheduled_state: &ScheduledState,
) -> Option<&'plan StateFlow> {
    lookups::state_flow_by_key(native_plan, scheduled_state.key).ok()
}

pub fn scheduled_state_key(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<StateKey> {
    let machine = lookups::machine_flow(native_plan, machine_name).ok()?;
    let state = lookups::state_flow(native_plan, machine, state_name).ok()?;

    Some(state.key)
}

pub(super) fn append_state_chain(
    native_plan: &NativePlan,
    start_key: StateKey,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(PlaceKey, String)>,
    aliases: &mut Vec<(PlaceKey, PlaceKey)>,
) -> Result<(), String> {
    let mut current_key = start_key;

    loop {
        let current = ScheduledState { key: current_key };

        if visited.contains(&current) {
            return Err(format!(
                "cycle {}; native emission does not support loops yet",
                cycle_path(native_plan, visited, &current)
            ));
        }

        visited.push(current.clone());
        schedule.push(current.clone());

        let machine = lookups::machine_flow_by_symbol(native_plan, current.key.machine)?;
        let state = lookups::state_flow_by_key(native_plan, current.key)?;
        append_local_state_calls(
            native_plan,
            machine,
            state,
            schedule,
            visited,
            values,
            aliases,
        )?;
        apply_static_operations(native_plan, state, aliases, values);

        let transitions = native_plan
            .control_flow
            .transitions
            .span(state.transitions)
            .unwrap_or(&[]);

        match transitions {
            [] => return Ok(()),
            transitions => {
                let transition = match select_static_transition(transitions, values, aliases) {
                    Some(Ok(transition)) => transition,
                    Some(Err(())) => {
                        return Err(format!(
                            "{} has a guard native emission cannot evaluate statically yet",
                            state_key_display(native_plan, current.key)
                        ));
                    }
                    None => {
                        return Err(format!(
                            "{} has no transition whose guard is satisfied",
                            state_key_display(native_plan, current.key)
                        ));
                    }
                };
                let Some(next_state) = next_state(
                    native_plan,
                    machine,
                    state,
                    transition,
                    schedule,
                    visited,
                    values,
                    aliases,
                )?
                else {
                    return Ok(());
                };

                current_key = next_state.key;
            }
        }
    }
}
