use crate::InstructionSelectionInput;
use omega_runtime_bodies::{RuntimeDispatchBodyOperation, RuntimeDispatchBodyOperationKind};
use omega_checked_trees::expression::ExpressionTable;
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
    match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall { .. }
        | RuntimeDispatchBodyOperationKind::InlineStateCall { .. }
        | RuntimeDispatchBodyOperationKind::StateCall { .. } => {}
        RuntimeDispatchBodyOperationKind::HostCall { .. }
        | RuntimeDispatchBodyOperationKind::LocalStorage { .. }
        | RuntimeDispatchBodyOperationKind::Mutation { .. }
        | RuntimeDispatchBodyOperationKind::StateCallResult { .. }
        | RuntimeDispatchBodyOperationKind::Other => return,
    }

    let (role, call_ordinal) = match &operation.kind {
        RuntimeDispatchBodyOperationKind::InlineLeafStateCall {
            role,
            call_ordinal,
            ..
        }
        | RuntimeDispatchBodyOperationKind::InlineStateCall {
            role,
            call_ordinal,
            ..
        }
        | RuntimeDispatchBodyOperationKind::StateCall {
            role,
            call_ordinal,
            ..
        } => (*role, *call_ordinal),
        _ => unreachable!(),
    };

    let state_call = match role {
        StateCallRole::Statement => {
            state_call_for_statement(input, operation.source_key, operation.statement_index)
        }
        StateCallRole::AssignmentValue => {
            state_assignment_value_call_by_ordinal(
                input,
                operation.source_key,
                operation.statement_index,
                call_ordinal,
            ).or_else(|| {
                state_assignment_value_call(input, operation.source_key, operation.statement_index)
            })
        }
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
        aliases.set_alias(
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
