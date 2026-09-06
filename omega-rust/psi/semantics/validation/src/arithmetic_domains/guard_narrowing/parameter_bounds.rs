//! Exact immutable operand bounds project through the current builtin guard.

use super::*;

pub(super) fn narrow(
    program: &TypedTrees,
    machine: &Machine,
    state: Option<&State>,
    environment: &mut ValueEnv,
    condition: ExpressionHandle,
    positive: bool,
) {
    let Some(state) = state else {
        return;
    };
    let ExpressionNode::Binary(comparison) = program.expression_table.expression(condition) else {
        return;
    };
    let Some((left, right)) = super::super::invariant_bounds::builtin_comparison_intervals(
        program, machine, state, condition,
    ) else {
        return;
    };
    let operator = if positive {
        comparison.operator
    } else {
        let Some(operator) = negate_comparison(comparison.operator) else {
            return;
        };
        operator
    };
    for (subject, subject_bounds, operand_bounds, subject_on_left) in [
        (comparison.left, left, right, true),
        (comparison.right, right, left, false),
    ] {
        // The bound owner already checked exact state-parameter custody and
        // immutability. Computed operands can supply a bound, not a new place.
        let ExpressionNode::Name(_) = program.expression_table.expression(subject) else {
            continue;
        };
        let Some(name) = place_path(program, subject) else {
            continue;
        };
        let interval = comparison_interval(operator, operand_bounds, subject_on_left)
            .intersect(subject_bounds);
        environment.narrow(name, interval);
    }
}
