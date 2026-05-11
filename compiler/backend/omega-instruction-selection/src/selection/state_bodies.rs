use crate::InstructionSelectionInput;
use omega_core::arena::Arena;
use omega_control_flow::{OperationKind, PlannedTransitionTarget, StateKey};
use omega_state_schedule::ScheduledState;
use omega_typed_trees::expression::ExpressionTable;

use super::bindings::{
    RuntimeAliasBinding, RuntimeAliasResolutionContext, resolve_runtime_alias_binding_handle,
    set_runtime_alias, strip_mutable_expression_handle,
};
use super::host_operations::select_host_call;
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::{host_call_for_statement, state_call_for_statement};
use omega_target_operations::InstructionOperand;
use omega_state_calls::StateCall;

pub(super) fn select_state_body_instructions(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
    dispatch_index: Option<u32>,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut Vec<StateKey>,
) {
    if visiting.contains(&state_key) {
        return;
    }

    visiting.push(state_key);

    let Some(state) = input.control_flow.state_by_key(state_key) else {
        visiting.pop();
        return;
    };
    let Some(operations) = input.control_flow.operations.span(state.operations) else {
        visiting.pop();
        return;
    };
    let transitions = input.control_flow.transitions.span_or_empty(state.transitions);

    for operation in operations {
        if let Some(host_call) =
            host_call_for_statement(input, state.key, operation.statement_index)
        {
            select_host_call(
                input,
                host_call,
                dispatch_index,
                Some(RuntimeAliasResolutionContext {
                    aliases,
                    alias_expressions,
                }),
                operands,
                selected_instructions,
            );
            continue;
        }

        let OperationKind::Call { .. } = &operation.kind else {
            continue;
        };
        let Some(state_call) =
            state_call_for_statement(input, state.key, operation.statement_index)
        else {
            continue;
        };

        if !state_call.target_key.is_valid() {
            continue;
        }

        let mut child_aliases = aliases.to_vec();
        let mut child_alias_expressions = alias_expressions.clone();
        bind_state_call_aliases(
            input,
            state_call,
            &mut child_aliases,
            &mut child_alias_expressions,
        );
        select_state_body_instructions(
            input,
            state_call.target_key,
            dispatch_index,
            &child_aliases,
            &child_alias_expressions,
            operands,
            selected_instructions,
            visiting,
        );
    }

    for transition in transitions {
        follow_transition_target(
            input,
            dispatch_index,
            aliases,
            alias_expressions,
            &transition.target,
            operands,
            selected_instructions,
            visiting,
        );
        if let Some(continuation) = &transition.continuation {
            follow_transition_target(
                input,
                dispatch_index,
                aliases,
                alias_expressions,
                continuation,
                operands,
                selected_instructions,
                visiting,
            );
        }
    }

    visiting.pop();
}

fn follow_transition_target(
    input: &InstructionSelectionInput<'_>,
    current_dispatch_index: Option<u32>,
    aliases: &[RuntimeAliasBinding],
    alias_expressions: &ExpressionTable,
    target: &PlannedTransitionTarget,
    operands: &mut Arena<InstructionOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut Vec<StateKey>,
) {
    let PlannedTransitionTarget::State { key, .. } = target else {
        return;
    };

    if !key.is_valid() {
        return;
    }

    select_state_body_instructions(
        input,
        *key,
        dispatch_index_for_state(input, *key).or(current_dispatch_index),
        aliases,
        alias_expressions,
        operands,
        selected_instructions,
        visiting,
    );
}

pub(super) fn runtime_reachable_states(
    input: &InstructionSelectionInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = Vec::new();

    for (_, state) in input.runtime_flow.states.iter() {
        push_scheduled_state_key(&mut states, state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        push_scheduled_state_key(&mut states, state_call.source_key);

        if state_call.target_key.is_valid() {
            push_scheduled_state_key(&mut states, state_call.target_key);
        }
    }

    states
}

fn push_scheduled_state_key(states: &mut Vec<ScheduledState>, key: omega_control_flow::StateKey) {
    if states
        .iter()
        .any(|scheduled_state| scheduled_state.key == key)
    {
        return;
    }

    states.push(ScheduledState { key });
}

fn bind_state_call_aliases(
    input: &InstructionSelectionInput<'_>,
    state_call: &StateCall,
    aliases: &mut Vec<RuntimeAliasBinding>,
    alias_expressions: &mut ExpressionTable,
) {
    let Some(arguments) = input.state_calls.arguments.span(state_call.arguments) else {
        return;
    };

    for argument in arguments {
        let argument_expression =
            alias_expressions.copy_from(&input.state_calls.expressions, argument.expression);
        let resolved_expression = resolve_runtime_alias_binding_handle(
            argument_expression,
            state_call.source_key,
            aliases,
            alias_expressions,
        );
        let expression =
            strip_mutable_expression_handle(alias_expressions, resolved_expression.expression);
        set_runtime_alias(
            aliases,
            RuntimeAliasBinding {
                source_key: state_call.target_key,
                parameter_symbol: argument.parameter_symbol,
                parameter_name: argument.parameter_name.clone(),
                expression_source_key: resolved_expression.source_key,
                expression,
            },
        );
    }
}

fn dispatch_index_for_state(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
) -> Option<u32> {
    input
        .runtime_bodies
        .bodies
        .iter()
        .find(|(_, body)| body.key == state_key)
        .map(|(_, body)| body.dispatch_index)
}
