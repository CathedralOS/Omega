use psi_typed_trees::expression::ExpressionHandle;

mod arguments;
mod guards;

use self::arguments::argument_is_parameter_tail_slice;
use self::guards::guard_is_non_empty_slice;
use super::patterns;

pub(super) fn state_has_proven_self_loop(
    program: &psi_typed_trees::TypedTrees,
    state: &psi_typed_trees::state::State,
    decreases: ExpressionHandle,
) -> bool {
    let Some(parameter) = patterns::parameter_matched_by_expression(program, state, decreases)
    else {
        return false;
    };
    let Some(parameter_index) = patterns::non_self_parameter_index(program, state, parameter)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            let Some(argument) = self_loop.arguments.get(parameter_index).copied() else {
                return false;
            };

            guard_is_non_empty_slice(program, self_loop.guard, parameter)
                && argument_is_parameter_tail_slice(program, argument, parameter)
        })
}
