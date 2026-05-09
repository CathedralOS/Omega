use crate::StateCallPlanningContext;
use omega_control_flow::StateKey;
use omega_core::symbols::SymbolHandle;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::lookups::state_flow_from_key;
use super::{StateCallArgument, StateCallArgumentKind};

pub(crate) fn build_call_arguments<'a>(
    context: &StateCallPlanningContext,
    target_key: StateKey,
    required: bool,
    raw_arguments: &'a [Expression],
) -> impl Iterator<Item = StateCallArgument> + 'a {
    let parameters = state_parameters(context, target_key);

    raw_arguments
        .iter()
        .enumerate()
        .map(move |(index, expression)| StateCallArgument {
            index,
            parameter_symbol: parameters
                .get(index)
                .map(|parameter| parameter.0)
                .unwrap_or_else(SymbolHandle::invalid),
            parameter_name: parameters
                .get(index)
                .map(|parameter| parameter.1.clone())
                .unwrap_or_default(),
            expression: expression.clone(),
            kind: if matches!(expression, Expression::Mutable(_)) {
                StateCallArgumentKind::MutableAlias
            } else {
                StateCallArgumentKind::Value
            },
            required,
        })
}

fn state_parameters(
    context: &StateCallPlanningContext,
    target_key: StateKey,
) -> Vec<(SymbolHandle, ProgramName)> {
    state_flow_from_key(context, target_key)
        .map(|state| {
            state
                .parameters
                .iter()
                .map(|parameter| (parameter.symbol, parameter.name.clone()))
                .collect()
        })
        .unwrap_or_default()
}
