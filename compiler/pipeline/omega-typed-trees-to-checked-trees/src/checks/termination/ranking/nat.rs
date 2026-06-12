use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

mod arguments;
mod guards;

use self::arguments::{
    argument_is_parameter_minus_one, argument_is_parameter_plus_one,
    argument_rebuilds_parameter_with_member_minus_one,
};
use self::guards::{
    guard_is_index_below_limit, guard_is_positive_parameter, guard_is_positive_parameter_member,
};
use super::DistanceOrientation;
use super::patterns;

pub(super) fn state_has_proven_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    orientation: DistanceOrientation,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            edge_decrease_proven(
                program,
                state,
                state,
                self_loop.guard,
                self_loop.arguments,
                decreases,
                orientation,
            )
        })
}

/// Prove that the Nat-descending measure strictly decreases across a single
/// guarded transition edge from `source` to `target`. For a self-loop this is
/// the classic countdown / distance proof; for a mutually-recursive cycle the
/// target differs from the source, so the decreasing argument is located by the
/// matching parameter name in the *target* state.
pub(super) fn edge_decrease_proven(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
    orientation: DistanceOrientation,
) -> bool {
    match program.expression_table.expression(decreases) {
        ExpressionNode::Name(_) => {
            countdown_edge(program, source, target, guard, arguments, decreases)
        }
        ExpressionNode::Member(_) => {
            member_countdown_edge(program, source, target, guard, arguments, decreases)
        }
        ExpressionNode::Binary(binary) if matches!(binary.operator, BinaryOperator::Subtract) => {
            // The bounded distance reads `upper - lower`. The swapped
            // orientation is the inverted-clause probe: it asks whether the
            // operands written the other way around would have proven.
            let (upper, lower) = match orientation {
                DistanceOrientation::Declared => (binary.left, binary.right),
                DistanceOrientation::Swapped => (binary.right, binary.left),
            };
            distance_edge(program, source, target, guard, arguments, upper, lower)
        }
        _ => false,
    }
}

/// Index of the non-self parameter with the given name in a state's parameter
/// list (the positional argument slot for that parameter on an incoming edge).
fn target_argument_index(
    program: &omega_typed_trees::TypedTrees,
    target: &omega_typed_trees::state::State,
    name: &str,
) -> Option<usize> {
    program
        .state_parameters(target)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| parameter.name.as_str() == name)
}

fn countdown_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> bool {
    let ExpressionNode::Name(decreases_path) = program.expression_table.expression(decreases)
    else {
        return false;
    };
    let decrease_name = program
        .expression_table
        .name_path_members(decreases_path.members)
        .last()
        .map(|member| member.as_str())
        .unwrap_or_default();
    let Some(parameter) = program
        .state_parameters(source)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .find(|parameter| {
            parameter.symbol == decreases_path.symbol || parameter.name.as_str() == decrease_name
        })
    else {
        return false;
    };
    let Some(argument_index) = target_argument_index(program, target, parameter.name.as_str())
    else {
        return false;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return false;
    };

    guard_is_positive_parameter(program, guard, parameter)
        && argument_is_parameter_minus_one(program, argument, parameter)
}

fn member_countdown_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    arguments: &[ExpressionHandle],
    decreases: ExpressionHandle,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(decreases) else {
        return false;
    };
    let Some(parameter) =
        patterns::parameter_matched_by_expression(program, source, member.receiver)
    else {
        return false;
    };
    let Some(argument_index) = target_argument_index(program, target, parameter.name.as_str())
    else {
        return false;
    };
    let Some(argument) = arguments.get(argument_index).copied() else {
        return false;
    };

    guard_is_positive_parameter_member(program, guard, parameter, member.member.as_str())
        && argument_rebuilds_parameter_with_member_minus_one(
            program,
            argument,
            parameter,
            member.member.as_str(),
        )
}

/// Prove the bounded distance `upper - lower` strictly decreases across one
/// guarded edge: the guard bounds `lower` below `upper`, `upper` is threaded
/// unchanged, and `lower` advances by one.
fn distance_edge(
    program: &omega_typed_trees::TypedTrees,
    source: &omega_typed_trees::state::State,
    target: &omega_typed_trees::state::State,
    guard: ExpressionHandle,
    arguments: &[ExpressionHandle],
    upper: ExpressionHandle,
    lower: ExpressionHandle,
) -> bool {
    let Some(limit_parameter) = patterns::parameter_matched_by_expression(program, source, upper)
    else {
        return false;
    };
    let Some(index_parameter) = patterns::parameter_matched_by_expression(program, source, lower)
    else {
        return false;
    };
    let Some(limit_index) = target_argument_index(program, target, limit_parameter.name.as_str())
    else {
        return false;
    };
    let Some(index_index) = target_argument_index(program, target, index_parameter.name.as_str())
    else {
        return false;
    };
    let Some(limit_argument) = arguments.get(limit_index).copied() else {
        return false;
    };
    let Some(index_argument) = arguments.get(index_index).copied() else {
        return false;
    };

    guard_is_index_below_limit(program, guard, index_parameter, limit_parameter)
        && patterns::expression_is_parameter(program, limit_argument, limit_parameter)
        && argument_is_parameter_plus_one(program, index_argument, index_parameter)
}
