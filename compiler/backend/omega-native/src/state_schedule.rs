use crate::control_flow::{
    MachineFlow, OperationKind, PlannedTransitionTarget, StateFlow, StateKey, TransitionFlow,
};
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

mod lookups;
mod static_values;

use lookups::{
    machine_flow, machine_flow_by_symbol, state_flow, state_flow_by_key, validate_state_index,
};
use static_values::{
    apply_static_operations, argument_binding_place_name, resolve_static_value,
    select_transition as select_static_transition, set_static_value,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledState {
    pub key: StateKey,
}

pub fn build_entry_state_schedule(native_plan: &NativePlan) -> Result<Vec<ScheduledState>, String> {
    let mut schedule = Vec::new();
    let mut visited = Vec::<ScheduledState>::new();
    let mut values = Vec::<(String, String)>::new();
    let mut aliases = Vec::<(String, String)>::new();

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
    state_flow_by_key(native_plan, scheduled_state.key).ok()
}

pub fn scheduled_state_key(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
) -> Option<StateKey> {
    let machine = machine_flow(native_plan, machine_name).ok()?;
    let state = state_flow(native_plan, machine, state_name).ok()?;

    Some(state.key)
}

fn append_state_chain(
    native_plan: &NativePlan,
    start_key: StateKey,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(String, String)>,
    aliases: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let mut current_key = start_key;

    loop {
        let current = ScheduledState { key: current_key };

        if visited.contains(&current) {
            return Err(format!(
                "cycle {}; native emission does not support loops yet",
                cycle_path(native_plan, &visited, &current)
            ));
        }

        visited.push(current.clone());
        schedule.push(current.clone());

        let machine = machine_flow_by_symbol(native_plan, current.key.machine)?;
        let state = state_flow_by_key(native_plan, current.key)?;
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

fn cycle_path(
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

fn state_key_display(native_plan: &NativePlan, key: StateKey) -> String {
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

fn append_local_state_calls(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    state: &StateFlow,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(String, String)>,
    aliases: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let Some(operations) = native_plan.control_flow.operations.span(state.operations) else {
        return Err(format!(
            "{}.{} has an invalid operation span",
            machine.name, state.name
        ));
    };

    for operation in operations {
        let OperationKind::Call {
            receiver,
            target,
            arguments,
        } = &operation.kind
        else {
            continue;
        };

        let is_platform_call = native_plan.host_calls.calls.iter().any(|(_, host_call)| {
            host_call.source_key == state.key
                && host_call.statement_index == operation.statement_index
        }) || native_plan.host_calls.unsupported_calls.iter().any(
            |(_, host_call)| {
                host_call.source_key == state.key
                    && host_call.statement_index == operation.statement_index
            },
        );

        if is_platform_call {
            continue;
        }

        let target_machine = resolve_state_call_machine(native_plan, machine, receiver.as_deref())
            .ok_or_else(|| {
                format!(
                    "{}.{} statement {} calls unknown state receiver `{}`",
                    machine.name,
                    state.name,
                    operation.statement_index,
                    receiver.as_deref().unwrap_or("self")
                )
            })?;

        let saved_alias_count = aliases.len();
        let saved_visited_count = visited.len();
        bind_state_arguments(
            native_plan,
            target_machine.as_str(),
            target.as_str(),
            arguments,
            aliases,
            values,
        )?;

        let target_machine_flow = machine_flow(native_plan, target_machine.as_str())?;
        let target_state_flow = state_flow(native_plan, target_machine_flow, target)?;
        append_state_chain(
            native_plan,
            target_state_flow.key,
            schedule,
            visited,
            values,
            aliases,
        )?;
        visited.truncate(saved_visited_count);
        aliases.truncate(saved_alias_count);
    }

    Ok(())
}

fn resolve_state_call_machine(
    native_plan: &NativePlan,
    machine: &MachineFlow,
    receiver: Option<&str>,
) -> Option<ProgramName> {
    let Some(receiver) = receiver else {
        return Some(machine.name.clone());
    };

    machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)
        .map(|contained| contained.type_name.clone())
        .or_else(|| {
            native_plan
                .control_flow
                .machines
                .iter()
                .find(|(_, candidate)| candidate.name == receiver)
                .map(|(_, candidate)| candidate.name.clone())
        })
}

fn bind_state_arguments(
    native_plan: &NativePlan,
    machine_name: &str,
    state_name: &str,
    arguments: &[Expression],
    aliases: &mut Vec<(String, String)>,
    values: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let machine = machine_flow(native_plan, machine_name)?;
    let state = state_flow(native_plan, machine, state_name)?;

    for (parameter, argument) in state.parameters.iter().zip(arguments) {
        let canonical_argument = argument_binding_place_name(argument, aliases);
        if let Some(canonical_argument) = canonical_argument {
            set_alias(aliases, parameter.to_string(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(argument, aliases, values) {
            set_static_value(values, parameter.to_string(), value);
        }
    }

    Ok(())
}

fn set_alias(aliases: &mut Vec<(String, String)>, parameter: String, target: String) {
    if let Some((_, existing_target)) = aliases
        .iter_mut()
        .find(|(existing_parameter, _)| existing_parameter == &parameter)
    {
        *existing_target = target;
    } else {
        aliases.push((parameter, target));
    }
}

fn next_state(
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
