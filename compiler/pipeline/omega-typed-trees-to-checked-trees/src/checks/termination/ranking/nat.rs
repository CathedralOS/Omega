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
use super::patterns;

pub(super) fn state_has_proven_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
) -> bool {
    match program.expression_table.expression(decreases) {
        ExpressionNode::Name(_) => state_has_proven_countdown_self_loop(program, state, decreases),
        ExpressionNode::Member(_) => {
            state_has_proven_member_countdown_self_loop(program, state, decreases)
        }
        ExpressionNode::Binary(binary) if matches!(binary.operator, BinaryOperator::Subtract) => {
            state_has_proven_distance_self_loop(program, state, *binary)
        }
        _ => false,
    }
}

fn state_has_proven_countdown_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
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
    let Some((parameter, argument_index)) = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find_map(|(index, parameter)| {
            (parameter.symbol == decreases_path.symbol || parameter.name.as_str() == decrease_name)
                .then_some((parameter, index))
        })
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            let Some(argument) = self_loop.arguments.get(argument_index).copied() else {
                return false;
            };

            guard_is_positive_parameter(program, self_loop.guard, parameter)
                && argument_is_parameter_minus_one(program, argument, parameter)
        })
}

fn state_has_proven_member_countdown_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
) -> bool {
    let ExpressionNode::Member(member) = program.expression_table.expression(decreases) else {
        return false;
    };
    let Some((parameter, argument_index)) =
        patterns::parameter_and_argument_index_matched_by_expression(
            program,
            state,
            member.receiver,
        )
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            let Some(argument) = self_loop.arguments.get(argument_index).copied() else {
                return false;
            };

            guard_is_positive_parameter_member(
                program,
                self_loop.guard,
                parameter,
                member.member.as_str(),
            ) && argument_rebuilds_parameter_with_member_minus_one(
                program,
                argument,
                parameter,
                member.member.as_str(),
            )
        })
}

fn state_has_proven_distance_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: omega_typed_trees::expression::TableBinaryExpression,
) -> bool {
    let Some(limit_parameter) =
        patterns::parameter_matched_by_expression(program, state, decreases.left)
    else {
        return false;
    };
    let Some(index_parameter) =
        patterns::parameter_matched_by_expression(program, state, decreases.right)
    else {
        return false;
    };
    let Some(limit_index) = patterns::non_self_parameter_index(program, state, limit_parameter)
    else {
        return false;
    };
    let Some(index_index) = patterns::non_self_parameter_index(program, state, index_parameter)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .filter_map(|statement| patterns::guarded_self_loop(program, state, statement))
        .any(|self_loop| {
            let Some(limit_argument) = self_loop.arguments.get(limit_index).copied() else {
                return false;
            };
            let Some(index_argument) = self_loop.arguments.get(index_index).copied() else {
                return false;
            };

            guard_is_index_below_limit(program, self_loop.guard, index_parameter, limit_parameter)
                && patterns::expression_is_parameter(program, limit_argument, limit_parameter)
                && argument_is_parameter_plus_one(program, index_argument, index_parameter)
        })
}
