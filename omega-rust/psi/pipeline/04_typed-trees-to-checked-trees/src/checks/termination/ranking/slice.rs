use typed_trees::expression::ExpressionHandle;

use super::patterns;

pub(super) fn state_has_proven_self_loop<'p>(
    program: &'p typed_trees::TypedTrees,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    decreases: ExpressionHandle,
    bound_lookup: &mut Option<validation::ImmutableBoundLookup<'p>>,
) -> bool {
    let Some(parameter) = patterns::parameter_matched_by_expression(program, state, decreases)
    else {
        return false;
    };
    if !validation::state_reference_parameter_binding_is_stable(
        program,
        machine,
        state,
        parameter.symbol,
    ) {
        return false;
    }
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

            validation::slice_tail_strictly_decreases_with_bound_lookup(
                program,
                self_loop.guard,
                argument,
                parameter,
                bound_lookup,
            )
        })
}
