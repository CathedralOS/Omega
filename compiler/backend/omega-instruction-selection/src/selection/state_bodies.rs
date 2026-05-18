use crate::InstructionSelectionInput;
use omega_checked_trees::expression::ExpressionTable;
use omega_control_flow::{OperationKind, PlannedTransitionTarget, StateKey};
use omega_core::arena::Arena;
use omega_state_schedule::{ScheduledState, ScheduledStateCollector};

use super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, RuntimeAliasResolutionContext,
    resolve_runtime_alias_binding, resolve_runtime_alias_binding_handle,
    strip_mutable_expression_handle,
};
use super::host_operations::select_host_call;
use super::instruction_sink::SelectedInstructionSink;
use super::lookups::{
    host_call_for_statement, state_call_for_statement, state_mutation_for_statement,
};
use super::runtime_dispatch::select_runtime_resolved_mutation_write;
use omega_state_calls::StateCall;
use omega_target_operations::{InstructionOperand, RuntimeValueOperand};

const INLINE_STATE_BODY_VISIT_COUNT: usize = 16;

pub(super) struct StateBodyVisitStack {
    inline: [Option<StateKey>; INLINE_STATE_BODY_VISIT_COUNT],
    len: usize,
    overflow: Vec<StateKey>,
}

impl StateBodyVisitStack {
    pub(super) fn new() -> Self {
        Self {
            inline: [None; INLINE_STATE_BODY_VISIT_COUNT],
            len: 0,
            overflow: Vec::new(),
        }
    }

    fn contains(&self, key: StateKey) -> bool {
        self.inline
            .iter()
            .take(self.len.min(INLINE_STATE_BODY_VISIT_COUNT))
            .flatten()
            .any(|candidate| *candidate == key)
            || self.overflow.contains(&key)
    }

    fn push(&mut self, key: StateKey) {
        if self.len < INLINE_STATE_BODY_VISIT_COUNT {
            self.inline[self.len] = Some(key);
        } else {
            self.overflow.push(key);
        }

        self.len += 1;
    }

    fn pop(&mut self) {
        if self.len == 0 {
            return;
        }

        self.len -= 1;
        if self.len < INLINE_STATE_BODY_VISIT_COUNT {
            self.inline[self.len] = None;
        } else {
            self.overflow.pop();
        }
    }
}

pub(super) fn select_state_body_instructions(
    input: &InstructionSelectionInput<'_>,
    state_key: StateKey,
    dispatch_index: Option<u32>,
    aliases: &RuntimeAliasBuffer,
    alias_expressions: &ExpressionTable,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut StateBodyVisitStack,
) {
    if visiting.contains(state_key) {
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
    let transitions = input
        .control_flow
        .transitions
        .span_or_empty(state.transitions);

    for operation in operations {
        if let Some(host_call) =
            host_call_for_statement(input, state.key, operation.statement_index)
        {
            select_host_call(
                input,
                host_call,
                dispatch_index,
                Some(RuntimeAliasResolutionContext {
                    aliases: aliases.bindings(),
                    alias_expressions,
                }),
                operands,
                selected_instructions,
            );
            continue;
        }

        if let Some(mutation) =
            state_mutation_for_statement(input, state.key, operation.statement_index)
        {
            let target = input.state_storage.expressions.to_tree(mutation.target);
            let value = input.state_storage.expressions.to_tree(mutation.value);
            let resolved_target = resolve_runtime_alias_binding(
                &target,
                state.key,
                aliases.bindings(),
                alias_expressions,
            );
            let resolved_value = resolve_runtime_alias_binding(
                &value,
                state.key,
                aliases.bindings(),
                alias_expressions,
            );
            if let Some(dispatch_index) =
                dispatch_index_for_state(input, state.key).or(dispatch_index)
            {
                let (machine_name, state_name) =
                    input.control_flow.state_names_by_key_cloned(state.key);
                select_runtime_resolved_mutation_write(
                    input,
                    dispatch_index,
                    state.key,
                    &machine_name,
                    &machine_name,
                    &state_name,
                    operation.statement_index,
                    &resolved_target.expression,
                    &resolved_value.expression,
                    runtime_value_operands,
                    selected_instructions,
                );
            }
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

        let mut child_aliases = RuntimeAliasBuffer::from_bindings(aliases.bindings());
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
            runtime_value_operands,
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
            runtime_value_operands,
            selected_instructions,
            visiting,
        );
        if transition.continuation != PlannedTransitionTarget::None {
            follow_transition_target(
                input,
                dispatch_index,
                aliases,
                alias_expressions,
                &transition.continuation,
                operands,
                runtime_value_operands,
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
    aliases: &RuntimeAliasBuffer,
    alias_expressions: &ExpressionTable,
    target: &PlannedTransitionTarget,
    operands: &mut Arena<InstructionOperand>,
    runtime_value_operands: &mut Arena<RuntimeValueOperand>,
    selected_instructions: &mut SelectedInstructionSink,
    visiting: &mut StateBodyVisitStack,
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
        runtime_value_operands,
        selected_instructions,
        visiting,
    );
}

pub(super) fn runtime_reachable_states(
    input: &InstructionSelectionInput<'_>,
) -> Vec<ScheduledState> {
    let mut states = ScheduledStateCollector::new();

    for (_, state) in input.runtime_flow.states.iter() {
        states.push_key(state.key);
    }

    for (_, state_call) in input.state_calls.calls.iter() {
        if !state_call.required {
            continue;
        }

        states.push_key(state_call.source_key);

        if state_call.target_key.is_valid() {
            states.push_key(state_call.target_key);
        }
    }

    states.finish()
}

fn bind_state_call_aliases(
    input: &InstructionSelectionInput<'_>,
    state_call: &StateCall,
    aliases: &mut RuntimeAliasBuffer,
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
            aliases.bindings(),
            alias_expressions,
        );
        let expression =
            strip_mutable_expression_handle(alias_expressions, resolved_expression.expression);
        aliases.set_alias(RuntimeAliasBinding {
            source_key: state_call.target_key,
            parameter_symbol: argument.parameter_symbol,
            parameter_name: argument.parameter_name.clone(),
            expression_source_key: resolved_expression.source_key,
            expression,
        });
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
