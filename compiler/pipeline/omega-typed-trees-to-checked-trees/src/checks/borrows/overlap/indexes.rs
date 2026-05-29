use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, TableRangeExpression};

pub(super) fn index_expressions_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: ExpressionHandle,
    right: ExpressionHandle,
) -> bool {
    if left == right {
        return true;
    }

    match (
        program.expression_table.expression(left),
        program.expression_table.expression(right),
    ) {
        (ExpressionNode::Integer(left_value), ExpressionNode::Integer(right_value)) => {
            left_value == right_value
        }
        (ExpressionNode::Range(left_range), ExpressionNode::Integer(right_value)) => {
            range_may_contain_integer(program, left_range, *right_value)
        }
        (ExpressionNode::Integer(left_value), ExpressionNode::Range(right_range)) => {
            range_may_contain_integer(program, right_range, *left_value)
        }
        (ExpressionNode::Range(left_range), ExpressionNode::Range(right_range)) => {
            ranges_may_overlap(program, left_range, right_range)
        }
        _ => true,
    }
}

fn range_may_contain_integer(
    program: &omega_typed_trees::TypedTrees,
    range: &TableRangeExpression,
    value: i64,
) -> bool {
    let (start, end) = range_integer_bounds(program, range);
    if start.is_some_and(|start| value < start) {
        return false;
    }
    if end.is_some_and(|end| value >= end) {
        return false;
    }
    true
}

fn ranges_may_overlap(
    program: &omega_typed_trees::TypedTrees,
    left: &TableRangeExpression,
    right: &TableRangeExpression,
) -> bool {
    let (left_start, left_end) = range_integer_bounds(program, left);
    let (right_start, right_end) = range_integer_bounds(program, right);

    if let (Some(left_end), Some(right_start)) = (left_end, right_start)
        && left_end <= right_start
    {
        return false;
    }
    if let (Some(right_end), Some(left_start)) = (right_end, left_start)
        && right_end <= left_start
    {
        return false;
    }
    true
}

fn range_integer_bounds(
    program: &omega_typed_trees::TypedTrees,
    range: &TableRangeExpression,
) -> (Option<i64>, Option<i64>) {
    (
        integer_expression_value(program, range.start),
        integer_expression_value(program, range.end),
    )
}

fn integer_expression_value(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<i64> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Integer(value) => Some(*value),
        _ => None,
    }
}
