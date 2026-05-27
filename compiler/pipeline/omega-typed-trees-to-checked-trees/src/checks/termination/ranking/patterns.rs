use omega_typed_trees::expression::{ExpressionHandle, ExpressionNode};
use omega_typed_trees::statement::{StatementNode, TransitionGuardNode, TransitionTargetNode};

pub(super) struct GuardedSelfLoop<'program> {
    pub(super) guard: ExpressionHandle,
    pub(super) arguments: &'program [ExpressionHandle],
}

pub(super) fn state_has_direct_self_loop(
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

pub(super) fn guarded_self_loop<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    statement: &StatementNode,
) -> Option<GuardedSelfLoop<'program>> {
    let StatementNode::Transition(transition) = statement else {
        return None;
    };
    let TransitionGuardNode::When(guard) = transition.guard else {
        return None;
    };
    let target = program.statement_table.transition_target(transition.target);
    let TransitionTargetNode::Named { path, arguments } = target else {
        return None;
    };
    if path.symbol != state.symbol {
        return None;
    }

    Some(GuardedSelfLoop {
        guard,
        arguments: program.statement_table.expression_handles(*arguments),
    })
}

pub(super) fn parameter_matched_by_expression<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state: &'program omega_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<&'program omega_typed_trees::signature::StateParameter> {
    program.state_parameters(state).iter().find(|parameter| {
        !parameter.is_self && expression_matches_parameter(program, expression, parameter)
    })
}

pub(super) fn parameter_and_argument_index_matched_by_expression<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state: &'program omega_typed_trees::state::State,
    expression: ExpressionHandle,
) -> Option<(
    &'program omega_typed_trees::signature::StateParameter,
    usize,
)> {
    program
        .state_parameters(state)
        .iter()
        .filter(|parameter| !parameter.is_self)
        .enumerate()
        .find_map(|(index, parameter)| {
            expression_matches_parameter(program, expression, parameter)
                .then_some((parameter, index))
        })
}

pub(super) fn non_self_parameter_index(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
    parameter: &omega_typed_trees::signature::StateParameter,
) -> Option<usize> {
    program
        .state_parameters(state)
        .iter()
        .filter(|candidate| !candidate.is_self)
        .position(|candidate| candidate.symbol == parameter.symbol)
}

pub(super) fn normalize_boolean_guard(
    program: &omega_typed_trees::TypedTrees,
    guard: ExpressionHandle,
) -> ExpressionHandle {
    match program.expression_table.expression(guard) {
        ExpressionNode::Binary(binary)
            if matches!(
                binary.operator,
                omega_typed_trees::expression::BinaryOperator::Equal
            ) && matches!(
                program.expression_table.expression(binary.right),
                ExpressionNode::Boolean(true)
            ) =>
        {
            binary.left
        }
        _ => guard,
    }
}

pub(super) fn expression_is_parameter(
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

pub(super) fn expression_is_parameter_member(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    parameter: &omega_typed_trees::signature::StateParameter,
    member_name: &str,
) -> bool {
    matches!(
        program.expression_table.expression(expression),
        ExpressionNode::Member(member)
            if member.member.as_str() == member_name
                && expression_is_parameter(program, member.receiver, parameter)
    )
}

pub(super) fn expression_matches_parameter(
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

fn branch_targets_state(
    target: &TransitionTargetNode,
    state_symbol: omega_core::symbols::SymbolHandle,
) -> bool {
    matches!(
        target,
        TransitionTargetNode::Named { path, .. } if path.symbol == state_symbol
    )
}
