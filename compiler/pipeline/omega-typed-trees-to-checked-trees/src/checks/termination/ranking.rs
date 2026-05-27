use omega_typed_trees::expression::{BinaryOperator, ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

use super::order::RankingOrder;

pub(super) fn machine_has_proven_supported_decrease(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> bool {
    let decreases = program
        .expression_table
        .expression_handles(machine.decreases);
    if decreases.len() != 1 {
        return false;
    }
    let decrease_order = program.machine_decrease_order(machine.decrease_order);

    let Some(order) = RankingOrder::from_path(decrease_order) else {
        return false;
    };

    program
        .machine_states(machine)
        .iter()
        .filter(|state| state_has_direct_self_loop(program, state))
        .all(|state| state_has_proven_supported_self_loop(program, state, decreases[0], order))
}

fn state_has_direct_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
) -> bool {
    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };

            branch_targets_state(
                program.statement_table.transition_target(transition.target),
                state.symbol,
            ) || branch_targets_state(
                program
                    .statement_table
                    .transition_target(transition.continuation),
                state.symbol,
            )
        })
}

fn state_has_proven_supported_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
    order: RankingOrder,
) -> bool {
    match order {
        RankingOrder::NatDescending => {
            match program.expression_table.expression(decreases) {
                ExpressionNode::Name(_) => {
                    state_has_proven_countdown_self_loop(program, state, decreases)
                }
                ExpressionNode::Binary(binary)
                    if matches!(binary.operator, BinaryOperator::Subtract) =>
                {
                    state_has_proven_distance_self_loop(program, state, *binary)
                }
                _ => false,
            }
        }
        RankingOrder::SliceLength => {
            state_has_proven_slice_length_self_loop(program, state, decreases)
        }
    }
}

fn state_has_proven_slice_length_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: ExpressionHandle,
) -> bool {
    let Some(parameter) = parameter_matched_by_expression(program, state, decreases) else {
        return false;
    };
    let non_self_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let Some(parameter_index) = non_self_parameters
        .iter()
        .position(|candidate| candidate.symbol == parameter.symbol)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return false;
            };
            let target = program.statement_table.transition_target(transition.target);
            let TransitionTargetNode::Named { path, arguments } = target else {
                return false;
            };
            if path.symbol != state.symbol {
                return false;
            }
            let arguments = program.statement_table.expression_handles(*arguments);
            let Some(argument) = arguments.get(parameter_index).copied() else {
                return false;
            };

            guard_is_non_empty_slice(program, guard, parameter)
                && argument_is_parameter_tail_slice(program, argument, parameter)
        })
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
    let parameters = program.state_parameters(state);
    let Some((parameter, argument_index)) = parameters
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
        .any(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return false;
            };
            let target = program.statement_table.transition_target(transition.target);
            let TransitionTargetNode::Named { path, arguments } = target else {
                return false;
            };
            if path.symbol != state.symbol {
                return false;
            }
            let arguments = program.statement_table.expression_handles(*arguments);
            let Some(argument) = arguments.get(argument_index).copied() else {
                return false;
            };

            guard_is_positive_parameter(program, guard, parameter)
                && argument_is_parameter_minus_one(program, argument, parameter)
        })
}

fn state_has_proven_distance_self_loop(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    decreases: omega_typed_trees::expression::TableBinaryExpression,
) -> bool {
    let Some(limit_parameter) = parameter_matched_by_expression(program, state, decreases.left)
    else {
        return false;
    };
    let Some(index_parameter) = parameter_matched_by_expression(program, state, decreases.right)
    else {
        return false;
    };
    let non_self_parameters = program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .collect::<Vec<_>>();
    let Some(limit_index) = non_self_parameters
        .iter()
        .position(|parameter| parameter.symbol == limit_parameter.symbol)
    else {
        return false;
    };
    let Some(index_index) = non_self_parameters
        .iter()
        .position(|parameter| parameter.symbol == index_parameter.symbol)
    else {
        return false;
    };

    program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .any(|statement| {
            let StatementNode::Transition(transition) = statement else {
                return false;
            };
            let TransitionGuardNode::When(guard) = transition.guard else {
                return false;
            };
            let target = program.statement_table.transition_target(transition.target);
            let TransitionTargetNode::Named { path, arguments } = target else {
                return false;
            };
            if path.symbol != state.symbol {
                return false;
            }
            let arguments = program.statement_table.expression_handles(*arguments);
            let Some(limit_argument) = arguments.get(limit_index).copied() else {
                return false;
            };
            let Some(index_argument) = arguments.get(index_index).copied() else {
                return false;
            };

            guard_is_index_below_limit(program, guard, index_parameter, limit_parameter)
                && expression_is_parameter(program, limit_argument, limit_parameter)
                && argument_is_parameter_plus_one(program, index_argument, index_parameter)
        })
}

fn parameter_matched_by_expression<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state: &'program omega_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<&'program omega_typed_trees::signature::StateParameter> {
    program.state_parameters(state).iter().find(|parameter| {
        !parameter.is_self && expression_matches_parameter(program, expression, parameter)
    })
}

fn branch_targets_state(
    target: &TransitionTargetNode,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> bool {
    matches!(
        target,
        TransitionTargetNode::Named { path, .. } if path.symbol == state_symbol
    )
}

fn guard_is_positive_parameter(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(0)
        )
}

fn guard_is_non_empty_slice(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Greater)
        && expression_matches_parameter_measure(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(0)
        )
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
        && expression_is_parameter(program, binary.left, parameter)
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
        && expression_is_parameter(program, binary.left, parameter)
        && matches!(
            program.expression_table.expression(binary.right),
            ExpressionNode::Integer(1)
        )
}

fn argument_is_parameter_tail_slice(
    program: &omega_typed_trees::TypedTrees,
    argument: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(argument) else {
        return false;
    };
    if !expression_is_parameter(program, indexed.collection, parameter) {
        return false;
    }
    let ExpressionNode::Range(range) = program.expression_table.expression(indexed.index) else {
        return false;
    };
    if !range.start.is_valid() {
        return false;
    }

    matches!(
        program.expression_table.expression(range.start),
        ExpressionNode::Integer(1)
    ) && !range.end.is_valid()
}

fn guard_is_index_below_limit(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
    index_parameter: &omega_typed_trees::signature::StateParameter,
    limit_parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let normalized = normalize_boolean_guard(program, guard);
    let ExpressionNode::Binary(binary) = program.expression_table.expression(normalized) else {
        return false;
    };
    matches!(binary.operator, BinaryOperator::Less)
        && expression_is_parameter(program, binary.left, index_parameter)
        && expression_matches_parameter_measure(program, binary.right, limit_parameter)
}

fn normalize_boolean_guard(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
) -> ExpressionHandle {
    match program.expression_table.expression(guard) {
        ExpressionNode::Binary(binary)
            if matches!(binary.operator, BinaryOperator::Equal)
                && matches!(
                    program.expression_table.expression(binary.right),
                    ExpressionNode::Boolean(true)
                ) =>
        {
            binary.left
        }
        _ => guard,
    }
}

fn expression_is_parameter(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return false;
    };

    path.symbol == parameter.symbol
        || program
            .expression_table
            .name_path_members(path.members)
            .last()
            .is_some_and(|member| member.as_str() == parameter.name.as_str())
}

fn expression_matches_parameter(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    expression_is_parameter(program, expression, parameter)
        || matches!(
            program.expression_table.expression(expression),
            ExpressionNode::Member(member)
                if member.member.as_str() == "len"
                    && expression_is_parameter(program, member.receiver, parameter)
        )
}

fn expression_matches_parameter_measure(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> bool {
    expression_matches_parameter(program, expression, parameter)
}
