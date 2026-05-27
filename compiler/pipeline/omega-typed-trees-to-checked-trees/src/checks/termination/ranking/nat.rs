use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};

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

fn guard_is_positive_parameter(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && patterns::expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(0)
        )
}

fn guard_is_positive_parameter_member(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, member_name)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(0)
        )
}

fn guard_is_index_below_limit(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    index_parameter: &omega_typed_trees::signature::StateParameter,
    limit_parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = patterns::normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Less)
        && patterns::expression_is_parameter(program, binary.left, index_parameter)
        && patterns::expression_matches_parameter(program, binary.right, limit_parameter)
}

fn argument_is_parameter_minus_one(
    program: &omega_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(argument) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Subtract)
        && patterns::expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(1)
        )
}

fn argument_is_parameter_plus_one(
    program: &omega_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(argument) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Add)
        && patterns::expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(1)
        )
}

fn argument_rebuilds_parameter_with_member_minus_one(
    program: &omega_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    let ExpressionNode::StructLiteral(struct_literal) =
        program.expression_table.expression(argument)
    else {
        return false;
    };

    program
        .expression_table
        .struct_fields(struct_literal.fields)
        .iter()
        .any(|field| {
            field.name.as_str() == member_name
                && argument_is_parameter_member_minus_one(
                    program,
                    field.value,
                    parameter,
                    member_name,
                )
        })
}

fn argument_is_parameter_member_minus_one(
    program: &omega_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    let ExpressionNode::Binary(binary) = program.expression_table.expression(argument) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Subtract)
        && patterns::expression_is_parameter_member(program, binary.left, parameter, member_name)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(1)
        )
}
