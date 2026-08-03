use super::append_state_chain;
use super::lookups::state_flow_by_key;
use super::model::StateScheduleWorkspace;
use super::static_values::{
    PlaceKey, argument_binding_place_key, resolve_static_value, set_static_value,
};
use crate::StateScheduleContext;
use omega_control_flow::{MachineFlow, StateFlow, StateKey};
use psi_arena::HandleSpan;
use psi_checked_trees::expression::ExpressionHandle;

pub(super) fn append_local_state_calls(
    context: &StateScheduleContext,
    _machine: &MachineFlow,
    state: &StateFlow,
    workspace: &mut StateScheduleWorkspace,
) -> Result<(), String> {
    for (_, state_call) in context.state_calls.calls.iter().filter(|(_, state_call)| {
        state_call.source_key == state.key && state_call.target_key.is_valid()
    }) {
        let target_state_key = state_call.target_key;
        let checkpoint = workspace.checkpoint();
        bind_state_call_arguments_by_key(
            context,
            target_state_key,
            state_call.arguments,
            workspace,
        )?;
        append_state_chain(context, target_state_key, workspace)?;
        workspace.restore(checkpoint);
    }

    Ok(())
}

pub(super) fn bind_state_arguments_by_key(
    context: &StateScheduleContext,
    state_key: StateKey,
    arguments: HandleSpan<ExpressionHandle>,
    workspace: &mut StateScheduleWorkspace,
) -> Result<(), String> {
    let state = state_flow_by_key(context, state_key)?;
    let arguments = context
        .control_flow
        .expressions
        .expression_handles(arguments);

    for (parameter, argument) in context
        .control_flow
        .state_parameters(state)
        .iter()
        .zip(arguments)
    {
        let canonical_argument = argument_binding_place_key(
            &context.control_flow.expressions,
            *argument,
            workspace.aliases(),
        );
        if let Some(canonical_argument) = canonical_argument {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            workspace.set_alias(parameter_key.clone(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(
            &context.control_flow.expressions,
            *argument,
            workspace.aliases(),
            workspace.values(),
        ) {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_static_value(workspace.values_mut(), parameter_key, value);
        }
    }

    Ok(())
}

fn bind_state_call_arguments_by_key(
    context: &StateScheduleContext,
    state_key: StateKey,
    arguments: HandleSpan<omega_state_calls::StateCallArgument>,
    workspace: &mut StateScheduleWorkspace,
) -> Result<(), String> {
    let state = state_flow_by_key(context, state_key)?;
    let arguments = context.state_calls.arguments.span(arguments).unwrap_or(&[]);

    for (parameter, argument) in context
        .control_flow
        .state_parameters(state)
        .iter()
        .zip(arguments)
    {
        let canonical_argument = argument_binding_place_key(
            &context.state_calls.expressions,
            argument.expression,
            workspace.aliases(),
        );
        if let Some(canonical_argument) = canonical_argument {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            workspace.set_alias(parameter_key.clone(), canonical_argument);
        }

        if let Some(value) = resolve_static_value(
            &context.state_calls.expressions,
            argument.expression,
            workspace.aliases(),
            workspace.values(),
        ) {
            let parameter_key =
                PlaceKey::from_symbol_name(parameter.symbol, parameter.name.clone());
            set_static_value(workspace.values_mut(), parameter_key, value);
        }
    }

    Ok(())
}
