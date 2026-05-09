use super::append_state_chain;
use super::local_calls::bind_state_arguments_by_key;
use super::lookups::{machine_flow_by_symbol, state_flow_by_key, validate_state_index};
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
            name: _,
            arguments,
        } => {
            validate_state_index(native_plan, machine, *index, &machine.name, &state.name)?;
            bind_state_arguments_by_key(native_plan, *key, arguments, aliases, values)?;
            Ok(Some(ScheduledState { key: *key }))
        }
        PlannedTransitionTarget::Terminal => Ok(None),
        PlannedTransitionTarget::SelfTarget => Err(format!(
            "{} self-transitions; native emission does not support loops yet",
            state.name
        )),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state: nested_state,
            arguments,
        } => {
            let nested_machine_symbol = machine
                .contains
                .iter()
                .find(|contained| {
                    if receiver_symbol.is_valid() {
                        contained.symbol == *receiver_symbol
                    } else {
                        contained.name == *receiver
                    }
                })
                .map(|contained| contained.type_symbol)
                .ok_or_else(|| {
                    format!(
                        "{}.{} transitions into unknown nested machine `{receiver}`",
                        machine.name, state.name
                    )
                })?;

            let saved_alias_count = aliases.len();
            let saved_visited_count = visited.len();
            let nested_machine_flow = machine_flow_by_symbol(native_plan, nested_machine_symbol)?;
            let nested_state_key = state_symbol
                .is_valid()
                .then(|| {
                    native_plan
                        .control_flow
                        .state_key_by_symbols(nested_machine_flow.symbol, *state_symbol)
                })
                .flatten()
                .or_else(|| {
                    native_plan
                        .control_flow
                        .states
                        .span(nested_machine_flow.states)
                        .and_then(|states| {
                            states
                                .iter()
                                .find(|candidate| candidate.name == *nested_state)
                                .map(|candidate| candidate.key)
                        })
                })
                .ok_or_else(|| {
                    format!(
                        "{}.{} transitions into unknown nested state `{receiver}.{nested_state}`",
                        machine.name, state.name
                    )
                })?;
            let nested_state_flow = state_flow_by_key(native_plan, nested_state_key)?;
            bind_state_arguments_by_key(
                native_plan,
                nested_state_flow.key,
                arguments,
                aliases,
                values,
            )?;
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
                    name: _,
                    arguments,
                }) => {
                    validate_state_index(native_plan, machine, *index, &machine.name, &state.name)?;
                    bind_state_arguments_by_key(native_plan, *key, arguments, aliases, values)?;
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
