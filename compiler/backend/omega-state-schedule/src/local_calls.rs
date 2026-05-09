use super::append_state_chain;
use super::lookups::{machine_flow_by_symbol, state_flow_by_key};
use super::model::ScheduledState;
use super::static_values::{
    PlaceKey, argument_binding_place_key, resolve_static_value, set_static_value,
};
use crate::StateScheduleContext;
use omega_control_flow::{MachineFlow, OperationKind, StateFlow, StateKey};
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

pub(super) fn append_local_state_calls(
    context: &StateScheduleContext,
    machine: &MachineFlow,
    state: &StateFlow,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(PlaceKey, String)>,
    aliases: &mut Vec<(PlaceKey, PlaceKey)>,
) -> Result<(), String> {
    let Some(operations) = context.control_flow.operations.span(state.operations) else {
        return Err(format!(
            "{}.{} has an invalid operation span",
            machine.name, state.name
        ));
    };

    for operation in operations {
        let OperationKind::Call {
            receiver_symbol,
            target_symbol,
            receiver,
            target,
            arguments,
        } = &operation.kind
        else {
            continue;
        };

        let is_platform_call = context.host_calls.calls.iter().any(|(_, host_call)| {
            host_call.source_key == state.key
                && host_call.statement_index == operation.statement_index
        }) || context.host_calls.unsupported_calls.iter().any(
            |(_, host_call)| {
                host_call.source_key == state.key
                    && host_call.statement_index == operation.statement_index
            },
        );

        if is_platform_call {
            continue;
        }

        let target_state_key = resolve_state_call_key(
            context,
            machine,
            *receiver_symbol,
            *target_symbol,
            receiver.as_ref(),
            target,
        )
        .ok_or_else(|| {
            format!(
                "{}.{} statement {} calls unknown state `{}` on receiver `{}`",
                machine.name,
                state.name,
                operation.statement_index,
                target,
                receiver.as_deref().unwrap_or("self")
            )
        })?;

        let saved_alias_count = aliases.len();
        let saved_visited_count = visited.len();
        bind_state_arguments_by_key(context, target_state_key, arguments, aliases, values)?;
        append_state_chain(
            context,
            target_state_key,
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

fn resolve_state_call_key(
    context: &StateScheduleContext,
    machine: &MachineFlow,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&ProgramName>,
    target: &ProgramName,
) -> Option<StateKey> {
    let target_machine =
        resolve_state_call_machine_flow(context, machine, receiver_symbol, receiver)?;

    if target_symbol.is_valid() {
        context
            .control_flow
            .state_key_by_symbols(target_machine.symbol, target_symbol)
    } else {
        let _ = target;
        None
    }
}

fn resolve_state_call_machine_flow<'plan>(
    context: &'plan StateScheduleContext,
    machine: &'plan MachineFlow,
    receiver_symbol: SymbolHandle,
    receiver: Option<&ProgramName>,
) -> Option<&'plan MachineFlow> {
    if receiver.is_none() || receiver.is_some_and(|receiver| receiver == "self") {
        return Some(machine);
    }

    if receiver_symbol.is_valid() {
        if let Some(contained) = machine
            .contains
            .iter()
            .find(|contained| contained.symbol == receiver_symbol)
        {
            return machine_flow_by_symbol(context, contained.type_symbol).ok();
        }

        return context.control_flow.machine_by_symbol(receiver_symbol);
    }

    let _ = receiver?;
    None
}

pub(super) fn bind_state_arguments_by_key(
    context: &StateScheduleContext,
    state_key: StateKey,
    arguments: &[Expression],
    aliases: &mut Vec<(PlaceKey, PlaceKey)>,
    values: &mut Vec<(PlaceKey, String)>,
) -> Result<(), String> {
    let state = state_flow_by_key(context, state_key)?;

    for (parameter, argument) in state.parameters.iter().zip(arguments) {
        let canonical_argument = argument_binding_place_key(argument, aliases);
        if let Some(canonical_argument) = canonical_argument {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_alias(aliases, parameter_key.clone(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(argument, aliases, values) {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_static_value(values, parameter_key, value);
        }
    }

    Ok(())
}

fn set_alias(aliases: &mut Vec<(PlaceKey, PlaceKey)>, parameter: PlaceKey, target: PlaceKey) {
    if let Some((_, existing_target)) = aliases
        .iter_mut()
        .find(|(existing_parameter, _)| existing_parameter == &parameter)
    {
        *existing_target = target;
    } else {
        aliases.push((parameter, target));
    }
}
