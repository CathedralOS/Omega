use super::append_state_chain;
use super::lookups::{
    machine_flow_by_symbol, state_flow_by_key, state_flow_by_machine_symbol_and_name,
};
use super::model::ScheduledState;
use super::static_values::{argument_binding_place_name, resolve_static_value, set_static_value};
use crate::control_flow::{MachineFlow, OperationKind, StateFlow, StateKey};
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;

pub(super) fn append_local_state_calls(
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

        let target_machine =
            resolve_state_call_machine_flow(native_plan, machine, receiver.as_deref()).ok_or_else(
                || {
                    format!(
                        "{}.{} statement {} calls unknown state receiver `{}`",
                        machine.name,
                        state.name,
                        operation.statement_index,
                        receiver.as_deref().unwrap_or("self")
                    )
                },
            )?;

        let saved_alias_count = aliases.len();
        let saved_visited_count = visited.len();
        let target_state_flow =
            state_flow_by_machine_symbol_and_name(native_plan, target_machine.symbol, target)?;
        bind_state_arguments_by_key(
            native_plan,
            target_state_flow.key,
            arguments,
            aliases,
            values,
        )?;
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

fn resolve_state_call_machine_flow<'plan>(
    native_plan: &'plan NativePlan,
    machine: &'plan MachineFlow,
    receiver: Option<&str>,
) -> Option<&'plan MachineFlow> {
    let Some(receiver) = receiver else {
        return Some(machine);
    };

    machine
        .contains
        .iter()
        .find(|contained| contained.name == receiver)
        .and_then(|contained| machine_flow_by_symbol(native_plan, contained.type_symbol).ok())
        .or_else(|| {
            native_plan
                .control_flow
                .machines
                .iter()
                .find(|(_, candidate)| candidate.name == receiver)
                .map(|(_, candidate)| candidate)
        })
}

pub(super) fn bind_state_arguments_by_key(
    native_plan: &NativePlan,
    state_key: StateKey,
    arguments: &[Expression],
    aliases: &mut Vec<(String, String)>,
    values: &mut Vec<(String, String)>,
) -> Result<(), String> {
    let state = state_flow_by_key(native_plan, state_key)?;

    for (parameter, argument) in state.parameters.iter().zip(arguments) {
        let canonical_argument = argument_binding_place_name(argument, aliases);
        if let Some(canonical_argument) = canonical_argument {
            set_alias(aliases, parameter.name.to_string(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(argument, aliases, values) {
            set_static_value(values, parameter.name.to_string(), value);
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
