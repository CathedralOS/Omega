use crate::InstructionSelectionInput;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_checked_trees::statement::StatementNode;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_state_calls::StateCallRole;

use super::super::bindings::{
    RuntimeAliasBinding, RuntimeAliasBuffer, resolve_runtime_alias_binding_handle,
    strip_mutable_expression_handle,
};
use super::super::lookups::{
    state_assignment_value_call, state_assignment_value_call_by_ordinal, state_call_for_statement,
    state_transition_argument_call, state_transition_argument_call_by_ordinal,
    state_transition_guard_call,
};

pub(super) fn bind_runtime_operation_aliases(
    input: &InstructionSelectionInput<'_>,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &mut RuntimeAliasBuffer,
    alias_expressions: &mut ExpressionTable,
) {
    bind_prior_local_aliases(input, operation, aliases, alias_expressions);

    match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
        | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
        RuntimeDispatchBodyOperationKind::HostCall
        | RuntimeDispatchBodyOperationKind::LocalStorage { .. }
        | RuntimeDispatchBodyOperationKind::Mutation { .. }
        | RuntimeDispatchBodyOperationKind::StateCallResult { .. }
        | RuntimeDispatchBodyOperationKind::Other => return,
    }

    let (role, call_ordinal) = match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
            role, call_ordinal, ..
        }
        | RuntimeDispatchBodyOperationKind::InlineStateCall {
            role, call_ordinal, ..
        }
        | RuntimeDispatchBodyOperationKind::StateCall {
            role, call_ordinal, ..
        } => (*role, *call_ordinal),
        _ => unreachable!(),
    };

    let state_call = match role {
        StateCallRole::Statement => {
            state_call_for_statement(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::AssignmentValue => state_assignment_value_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_assignment_value_call(input, operation.source_key, operation.statement_index)
        }),
        StateCallRole::TransitionGuard => {
            state_transition_guard_call(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::TransitionArgument => state_transition_argument_call_by_ordinal(
            input,
            operation.source_key,
            operation.statement_index,
            call_ordinal,
        )
        .or_else(|| {
            state_transition_argument_call(input, operation.source_key, operation.statement_index)
        }),
        _ => None,
    };
    let Some(state_call) = state_call else {
        return;
    };
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

fn bind_prior_local_aliases(
    input: &InstructionSelectionInput<'_>,
    operation: &RuntimeDispatchBodyOperation,
    aliases: &mut RuntimeAliasBuffer,
    alias_expressions: &mut ExpressionTable,
) {
    let Some(machine) = input
        .program
        .machines()
        .iter()
        .find(|machine| machine.symbol == operation.source_key.machine)
    else {
        return;
    };
    let Some(state) = input
        .program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == operation.source_key.state)
    else {
        return;
    };

    let statements = input
        .program
        .statement_table
        .statements(state.statement_nodes);
    for (statement_index, statement) in statements
        .iter()
        .enumerate()
        .take(operation.statement_index)
    {
        let StatementNode::LocalData(local_data) = statement else {
            continue;
        };
        if !local_data.initial_value.is_valid()
            || local_requires_runtime_storage(
                input,
                operation.source_key,
                statement_index,
                local_data.symbol,
            )
            || local_is_assigned_between(
                input,
                statements,
                statement_index + 1,
                operation.statement_index,
                local_data.symbol,
            )
        {
            continue;
        }

        let initializer =
            alias_expressions.copy_from(&input.program.expression_table, local_data.initial_value);
        let resolved_initializer = resolve_runtime_alias_binding_handle(
            initializer,
            operation.source_key,
            aliases.bindings(),
            alias_expressions,
        );
        aliases.set_alias(RuntimeAliasBinding {
            source_key: operation.source_key,
            parameter_symbol: local_data.symbol,
            parameter_name: local_data.name.clone(),
            expression_source_key: resolved_initializer.source_key,
            expression: resolved_initializer.expression,
        });
    }
}

fn local_requires_runtime_storage(
    input: &InstructionSelectionInput<'_>,
    source_key: StateKey,
    statement_index: usize,
    symbol: SymbolHandle,
) -> bool {
    input.state_storage.locals.iter().any(|(_, local)| {
        local.source_key == source_key
            && local.statement_index == statement_index
            && local.symbol == symbol
    })
}

fn local_is_assigned_between(
    input: &InstructionSelectionInput<'_>,
    statements: &[StatementNode],
    start: usize,
    end: usize,
    symbol: SymbolHandle,
) -> bool {
    statements
        .iter()
        .skip(start)
        .take(end.saturating_sub(start))
        .any(|statement| {
            let StatementNode::Assignment(assignment) = statement else {
                return false;
            };
            assignment_target_head_symbol(input, assignment.target) == symbol
        })
}

fn assignment_target_head_symbol(
    input: &InstructionSelectionInput<'_>,
    expression: ExpressionHandle,
) -> SymbolHandle {
    match input.program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => path.head_symbol,
        ExpressionNode::Member(member) => assignment_target_head_symbol(input, member.receiver),
        ExpressionNode::Indexed(indexed) => {
            assignment_target_head_symbol(input, indexed.collection)
        }
        ExpressionNode::Mutable(inner) => assignment_target_head_symbol(input, *inner),
        _ => SymbolHandle::invalid(),
    }
}
