use super::append_state_chain;
use super::lookups::{machine_flow, state_flow};
use super::model::ScheduledState;
use super::static_values::{argument_binding_place_name, resolve_static_value, set_static_value};
use crate::control_flow::{MachineFlow, OperationKind, StateFlow};
use crate::plan::NativePlan;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

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

pub(super) fn bind_state_arguments(
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
