use crate::StateCallPlanningContext;
use omega_control_flow::StateKey;
use omega_control_flow::StateParameterFlow;
use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;

use super::lookups::state_flow_from_key;
use super::{StateCallArgument, StateCallArgumentKind, StateCallRole};

pub(crate) fn build_call_arguments(
    context: &StateCallPlanningContext,
    output_expressions: &mut ExpressionTable,
    output_arguments: &mut Arena<StateCallArgument>,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    target_key: StateKey,
    required: bool,
    raw_arguments: HandleSpan<ExpressionHandle>,
) -> HandleSpan<StateCallArgument> {
    let parameters = state_parameters(context, target_key);
    let raw_arguments = context
        .control_flow
        .expressions
        .expression_handles(raw_arguments);

    output_arguments.insert_many(raw_arguments.iter().enumerate().map(
        move |(index, expression)| {
            StateCallArgument {
                index,
                parameter_symbol: parameters
                    .get(index)
                    .map(|parameter| parameter.symbol)
                    .unwrap_or_else(SymbolHandle::invalid),
                parameter_name: parameters
                    .get(index)
                    .map(|parameter| parameter.name.clone())
                    .unwrap_or_default(),
                expression: output_expressions
                    .copy_from(&context.control_flow.expressions, *expression),
                kind: if argument_is_mutable_alias(
                    context,
                    source_key,
                    statement_index,
                    role,
                    index,
                    *expression,
                ) || parameters
                    .get(index)
                    .is_some_and(|parameter| parameter.is_mutable_reference)
                {
                    StateCallArgumentKind::MutableAlias
                } else {
                    StateCallArgumentKind::Value
                },
                required,
            }
        },
    ))
}

fn state_parameters(
    context: &StateCallPlanningContext,
    target_key: StateKey,
) -> &[StateParameterFlow] {
    state_flow_from_key(context, target_key)
        .map(|state| state.parameters.as_slice())
        .unwrap_or_default()
}

fn argument_is_mutable_alias(
    context: &StateCallPlanningContext,
    source_key: StateKey,
    statement_index: usize,
    role: StateCallRole,
    index: usize,
    expression: ExpressionHandle,
) -> bool {
    if role == StateCallRole::Statement
        && let Some(call) = context.borrow_call_by_key(source_key, statement_index)
        && let Some(access) = context
            .control_flow
            .borrow_argument_accesses
            .span(call.accesses)
            .and_then(|accesses| accesses.get(index))
    {
        return access.kind == omega_control_flow::StateBorrowAccessKind::Mutable;
    }

    matches!(
        context.control_flow.expressions.expression(expression),
        ExpressionNode::Mutable(_)
    )
}
