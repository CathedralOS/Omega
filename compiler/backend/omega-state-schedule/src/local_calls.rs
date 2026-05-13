use super::append_state_chain;
use super::lookups::state_flow_by_key;
use super::model::ScheduledState;
use super::static_values::{
    PlaceKey, argument_binding_place_key, resolve_static_value, set_static_value,
};
use crate::StateScheduleContext;
use omega_checked_trees::expression::ExpressionHandle;
use omega_control_flow::{MachineFlow, StateFlow, StateKey};
use omega_core::arena::HandleSpan;

pub(super) fn append_local_state_calls(
    context: &StateScheduleContext,
    _machine: &MachineFlow,
    state: &StateFlow,
    schedule: &mut Vec<ScheduledState>,
    visited: &mut Vec<ScheduledState>,
    values: &mut Vec<(PlaceKey, String)>,
    aliases: &mut Vec<(PlaceKey, PlaceKey)>,
) -> Result<(), String> {
    for (_, state_call) in context.state_calls.calls.iter().filter(|(_, state_call)| {
        state_call.source_key == state.key && state_call.target_key.is_valid()
    }) {
        let target_state_key = state_call.target_key;
        let saved_alias_count = aliases.len();
        let saved_visited_count = visited.len();
        bind_state_call_arguments_by_key(
            context,
            target_state_key,
            state_call.arguments,
            aliases,
            values,
        )?;
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

pub(super) fn bind_state_arguments_by_key(
    context: &StateScheduleContext,
    state_key: StateKey,
    arguments: HandleSpan<ExpressionHandle>,
    aliases: &mut Vec<(PlaceKey, PlaceKey)>,
    values: &mut Vec<(PlaceKey, String)>,
) -> Result<(), String> {
    let state = state_flow_by_key(context, state_key)?;
    let arguments = context
        .control_flow
        .expressions
        .expression_handles(arguments);

    for (parameter, argument) in state.parameters.iter().zip(arguments) {
        let canonical_argument =
            argument_binding_place_key(&context.control_flow.expressions, *argument, aliases);
        if let Some(canonical_argument) = canonical_argument {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_alias(aliases, parameter_key.clone(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(
            &context.control_flow.expressions,
            *argument,
            aliases,
            values,
        ) {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_static_value(values, parameter_key, value);
        }
    }

    Ok(())
}

fn bind_state_call_arguments_by_key(
    context: &StateScheduleContext,
    state_key: StateKey,
    arguments: HandleSpan<omega_state_calls::StateCallArgument>,
    aliases: &mut Vec<(PlaceKey, PlaceKey)>,
    values: &mut Vec<(PlaceKey, String)>,
) -> Result<(), String> {
    let state = state_flow_by_key(context, state_key)?;
    let arguments = context
        .state_calls
        .arguments
        .span(arguments)
        .unwrap_or(&[]);

    for (parameter, argument) in state.parameters.iter().zip(arguments) {
        let canonical_argument =
            argument_binding_place_key(&context.state_calls.expressions, argument.expression, aliases);
        if let Some(canonical_argument) = canonical_argument {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_alias(aliases, parameter_key.clone(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(
            &context.state_calls.expressions,
            argument.expression,
            aliases,
            values,
        ) {
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
