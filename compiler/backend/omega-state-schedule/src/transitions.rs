use super::append_state_chain;
use super::local_calls::bind_state_arguments_by_key;
use super::lookups::{machine_flow_by_symbol, state_flow_by_key, validate_state_index};
use super::model::{ScheduledState, StateScheduleWorkspace};
use crate::StateScheduleContext;
use omega_control_flow::{MachineFlow, PlannedTransitionTarget, StateFlow, TransitionFlow};

pub(super) fn next_state(
    context: &StateScheduleContext,
    machine: &MachineFlow,
    state: &StateFlow,
    transition: &TransitionFlow,
    workspace: &mut StateScheduleWorkspace,
) -> Result<Option<ScheduledState>, String> {
    match &transition.target {
        PlannedTransitionTarget::State {
            index,
            key,
            name: _,
        } => {
            validate_state_index(context, machine, *index, &machine.name, &state.name)?;
            bind_state_arguments_by_key(
                context,
                *key,
                transition.expressions.target_arguments,
                workspace,
            )?;
            Ok(Some(ScheduledState { key: *key }))
        }
        PlannedTransitionTarget::None | PlannedTransitionTarget::Terminal => Ok(None),
        PlannedTransitionTarget::SelfTarget => Err(format!(
            "{} self-transitions; native emission does not support loops yet",
            state.name
        )),
        PlannedTransitionTarget::Nested {
            receiver_symbol,
            state_symbol,
            receiver,
            state: nested_state,
        } => {
            let nested_machine_symbol = context
                .control_flow
                .machine_contains(machine)
                .iter()
                .find(|contained| {
                    receiver_symbol.is_valid() && contained.symbol == *receiver_symbol
                })
                .map(|contained| contained.type_symbol)
                .ok_or_else(|| {
                    format!(
                        "{}.{} transitions into unknown nested machine `{receiver}`",
                        machine.name, state.name
                    )
                })?;

            let checkpoint = workspace.checkpoint();
            let nested_machine_flow = machine_flow_by_symbol(context, nested_machine_symbol)?;
            let nested_state_key = if state_symbol.is_valid() {
                context
                    .control_flow
                    .state_key_by_symbols(nested_machine_flow.symbol, *state_symbol)
            } else {
                let _ = nested_state;
                None
            }
            .ok_or_else(|| {
                format!(
                    "{}.{} transitions into unknown nested state `{receiver}.{nested_state}`",
                    machine.name, state.name
                )
            })?;
            let nested_state_flow = state_flow_by_key(context, nested_state_key)?;
            bind_state_arguments_by_key(
                context,
                nested_state_flow.key,
                transition.expressions.target_arguments,
                workspace,
            )?;
            append_state_chain(context, nested_state_flow.key, workspace)?;
            workspace.restore(checkpoint);

            match &transition.continuation {
                PlannedTransitionTarget::State {
                    index,
                    key,
                    name: _,
                } => {
                    validate_state_index(context, machine, *index, &machine.name, &state.name)?;
                    bind_state_arguments_by_key(
                        context,
                        *key,
                        transition.expressions.continuation_arguments,
                        workspace,
                    )?;
                    Ok(Some(ScheduledState { key: *key }))
                }
                PlannedTransitionTarget::Terminal | PlannedTransitionTarget::None => Ok(None),
                PlannedTransitionTarget::SelfTarget => Err(format!(
                    "{}.{} nested continuation self-transitions; native emission does not support loops yet",
                    machine.name, state.name
                )),
                PlannedTransitionTarget::Nested {
                    receiver,
                    state: nested_state,
                    ..
                } => Err(format!(
                    "{}.{} nested continuation targets `{receiver}.{nested_state}`; native emission supports one nested call at a time so far",
                    machine.name, state.name
                )),
            }
        }
    }
}
