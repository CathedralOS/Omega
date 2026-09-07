use super::*;
use typed_trees::statement::TableTransition;

/// Analyze an exact target-arm argument without importing facts from another occurrence.
/// Selected operators and effectful argument trees need evaluated snapshots;
/// a call-free spelling alone does not establish that an operator is builtin.
pub(super) fn integer_bounds(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    transition: &TableTransition,
    expression: ExpressionHandle,
) -> Option<(i64, i64)> {
    let TransitionTargetNode::Named { arguments, .. } =
        program.statement_table.transition_target(transition.target)
    else {
        return None;
    };
    let arguments = program.statement_table.expression_handles(*arguments);
    if !arguments.contains(&expression)
        || !arguments
            .iter()
            .all(|argument| stable_builtin_value(program, machine, state, *argument, 0))
    {
        return None;
    }
    // Declared operand bounds remain obligations of every write/arrival.
    // No predecessor guard or body-local inferred value is assumed here.
    let mut environment = ValueEnv::new();
    if let TransitionGuardNode::When(condition) = transition.guard {
        if !stable_builtin_value(program, machine, state, condition, 0) {
            return None;
        }
        narrow_env_by_condition(
            program,
            machine,
            Some(state),
            &mut environment,
            condition,
            true,
        );
    }
    let mut diagnostics = Vec::new();
    let value = analyze(
        program,
        machine,
        Some(state),
        expression,
        &environment,
        None,
        ArithmeticDomain::Exact,
        "transition argument",
        &mut diagnostics,
    );
    let (minimum, maximum) = (value.interval.low?, value.interval.high?);
    (diagnostics.is_empty() && minimum <= maximum).then_some((minimum, maximum))
}

fn stable_builtin_value(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    expression: ExpressionHandle,
    depth: usize,
) -> bool {
    if depth >= 128 || !program.expression_table.expression_is_valid(expression) {
        return false;
    }
    let stable = |value| stable_builtin_value(program, machine, state, value, depth + 1);
    match program.expression_table.expression(expression) {
        ExpressionNode::Name(path) => {
            if !path.symbol.is_valid() || path.head_symbol != path.symbol {
                return false;
            }
            let is_parameter = program
                .state_parameters(state)
                .iter()
                .any(|parameter| parameter.symbol == path.symbol);
            let is_local = program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .any(|statement| {
                    matches!(statement,
                    StatementNode::LocalData(local) if local.symbol == path.symbol)
                });
            is_parameter || is_local
        }
        ExpressionNode::Integer(_) | ExpressionNode::Boolean(_) => true,
        ExpressionNode::Binary(binary) => {
            // Anonymous division retains rational intermediates until landing;
            // the integer interval engine must not replace it with truncation.
            let division_is_not_anonymous = binary.operator != BinaryOperator::Divide
                || crate::literals::anonymous_numeric_value(program, expression, &mut |value| {
                    crate::literals::has_anonymous_operator_meaning(program, value)
                })
                .is_none();
            division_is_not_anonymous
                && meaning::has_builtin_decomposed_guard_meaning(
                    program,
                    machine,
                    Some(state),
                    expression,
                )
                && stable(binary.left)
                && stable(binary.right)
        }
        ExpressionNode::Unary(unary)
            if unary.operator == typed_trees::expression::UnaryOperator::LogicalNot =>
        {
            stable(unary.operand)
        }
        ExpressionNode::Atomic(atomic) => stable(atomic.value),
        // Calls, projections and conversions retain their existing snapshot
        // and selected-meaning owners; this query supplies no new evidence.
        _ => false,
    }
}
