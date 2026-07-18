use omega_typed_trees::expression::ExpressionHandle;

mod arguments;
mod guards;

use self::arguments::argument_is_parameter_tail_slice;
use self::guards::guard_is_non_empty_slice;
use super::{EdgeClass, patterns};

pub(super) fn state_has_proven_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
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

pub(super) fn classify_cross_machine_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> EdgeClass {
    let Some(parameter) = patterns::parameter_matched_by_expression(program, source, decreases)
    else {
        return EdgeClass::Unknown;
    };
    let Some(argument_index) = program
        .state_parameters(target)
        .iter()
        .filter(|candidate| !candidate.is_self)
        .position(|candidate| candidate.name == parameter.name)
    else {
        return EdgeClass::Unknown;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return EdgeClass::Unknown;
    };

    if guard_is_non_empty_slice(program, guard, parameter)
        && argument_is_parameter_tail_slice(program, argument, parameter)
    {
        EdgeClass::Strict
    } else if patterns::expression_is_parameter(program, argument, parameter) {
        EdgeClass::NonIncreasing
    } else {
        EdgeClass::Unknown
    }
}
