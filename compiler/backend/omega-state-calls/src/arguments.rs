use crate::StateCallPlanningContext;
use omega_control_flow::StateKey;
use omega_control_flow::StateParameterFlow;
use omega_core::arena::{Arena, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode, ExpressionTable};

use super::lookups::state_flow_from_key;
use super::{StateCallArgument, StateCallArgumentKind};

pub(crate) fn build_call_arguments(
    context: &StateCallPlanningContext,
    output_expressions: &mut ExpressionTable,
    output_arguments: &mut Arena<StateCallArgument>,
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
                kind: if matches!(
                    context.control_flow.expressions.expression(*expression),
                    ExpressionNode::Mutable(_)
                ) {
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
