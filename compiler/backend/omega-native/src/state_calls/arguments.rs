use crate::control_flow::StateKey;
use crate::state_analysis::StateAnalysisContext;
use omega_typed_program::expression::Expression;
use omega_typed_program::name::ProgramName;

use super::lookups::state_flow_from_key;
use super::{StateCallArgument, StateCallArgumentKind};

pub(in crate::state_calls) fn build_call_arguments<'a>(
    context: &StateAnalysisContext,
    target_key: StateKey,
    required: bool,
    raw_arguments: &'a [Expression],
) -> impl Iterator<Item = StateCallArgument> + 'a {
    let parameter_names = state_parameter_names(context, target_key);

    raw_arguments
        .iter()
        .enumerate()
        .map(move |(index, expression)| StateCallArgument {
            index,
            parameter_name: parameter_names.get(index).cloned().unwrap_or_default(),
            expression: expression.clone(),
            kind: if matches!(expression, Expression::Mutable(_)) {
                StateCallArgumentKind::MutableAlias
            } else {
                StateCallArgumentKind::Value
            },
            required,
        })
}

fn state_parameter_names(context: &StateAnalysisContext, target_key: StateKey) -> Vec<ProgramName> {
    state_flow_from_key(context, target_key)
        .map(|state| {
            state
                .parameters
                .iter()
                .map(|parameter| parameter.clone())
                .collect()
        })
        .unwrap_or_default()
}
