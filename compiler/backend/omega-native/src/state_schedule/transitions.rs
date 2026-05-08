use super::append_state_chain;
use super::local_calls::bind_state_arguments;
use super::lookups::{machine_flow, state_flow, validate_state_index};
use super::model::ScheduledState;
use crate::control_flow::{MachineFlow, PlannedTransitionTarget, StateFlow, TransitionFlow};
use crate::plan::NativePlan;

pub(super) fn next_state(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    state: &StateFlow,
    transition: &TransitionFlow,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(String, String)>,
    aliases: &mut Vec<(String, String)>,
) -> Result<Option<ScheduledState>, String> {
    match &transition.target {
        PlannedTransitionTarget::State {
            index,
            key,
            name,
            arguments,
        } => {
            validate_state_index(native_plan, machine, *index, &machine.name, &state.name)?;
            bind_state_arguments(native_plan, &machine.name, name, arguments, aliases, values)?;
            Ok(Some(ScheduledState { key: *key }))
        }
        PlannedTransitionTarget::Terminal => Ok(None),
        PlannedTransitionTarget::SelfTarget => Err(format!(
            "{} self-transitions; native emission does not support loops yet",
            state.name
        )),
        PlannedTransitionTarget::Nested {
            receiver,
            state: nested_state,
            arguments,
        } => {
            let nested_machine_name = machine
                .contains
                .iter()
                .find(|contained| contained.name == *receiver)
                .map(|contained| contained.type_name.as_str())
                .ok_or_else(|| {
                    format!(
                        "{}.{} transitions into unknown nested machine `{receiver}`",
                        machine.name, state.name
                    )
                })?;

            let saved_alias_count = aliases.len();
            let saved_visited_count = visited.len();
            bind_state_arguments(
                native_plan,
                nested_machine_name,
                nested_state,
                arguments,
                aliases,
                values,
            )?;
            let nested_machine_flow = machine_flow(native_plan, nested_machine_name)?;
            let nested_state_flow = state_flow(native_plan, nested_machine_flow, nested_state)?;
            append_state_chain(
                native_plan,
                nested_state_flow.key,
                schedule,
                visited,
                values,
                aliases,
            )?;
            visited.truncate(saved_visited_count);
            aliases.truncate(saved_alias_count);

            match &transition.continuation {
                Some(PlannedTransitionTarget::State {
                    index,
                    key,
                    name,
                    arguments,
                }) => {
                    validate_state_index(native_plan, machine, *index, &machine.name, &state.name)?;
                    bind_state_arguments(
                        native_plan,
                        &machine.name,
                        name,
                        arguments,
                        aliases,
                        values,
                    )?;
                    Ok(Some(ScheduledState { key: *key }))
                }
                Some(PlannedTransitionTarget::Terminal) | None => Ok(None),
                Some(PlannedTransitionTarget::SelfTarget) => Err(format!(
                    "{}.{} nested continuation self-transitions; native emission does not support loops yet",
                    machine.name, state.name
                )),
                Some(PlannedTransitionTarget::Nested {
                    receiver,
                    state: nested_state,
                    ..
                }) => Err(format!(
                    "{}.{} nested continuation targets `{receiver}.{nested_state}`; native emission supports one nested call at a time so far",
                    machine.name, state.name
                )),
            }
        }
    }
}
