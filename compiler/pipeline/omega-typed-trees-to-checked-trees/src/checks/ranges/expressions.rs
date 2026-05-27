use omega_core::symbols::SymbolHandle;
use omega_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableRangeExpression,
};

use super::facts::RangeFacts;

pub(super) fn provable_range_bounds(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
) -> Option<(i64, Option<i64>)> {
    let start = if range.start.is_valid() {
        expression_integer_value(program, facts, range.start)?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        Some(expression_integer_value(program, facts, range.end)?)
    } else {
        None
    };
    Some((start, end))
}

pub(super) fn expression_integer_value(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<i64> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let left = expression_integer_value(program, facts, binary.left)?;
            let right = expression_integer_value(program, facts, binary.right)?;
            folded_integer_binary(left, binary.operator, right)
        }
        ExpressionNode::Integer(value) => Some(*value),
        ExpressionNode::Name(_) => {
            let (symbol, name) = expression_name(program, expression)?;
            facts.local_integer(symbol, name)
        }
        _ => None,
    }
}

pub(super) fn expression_name(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<(SymbolHandle, Option<&str>)> {
    let ExpressionNode::Name(path) = program.expression_table.expression(expression) else {
        return None;
    };
    Some((
        path.symbol,
        program
            .expression_table
            .name_path_members(path.members)
            .last()
            .map(|name| name.as_str()),
    ))
}

pub(super) fn expression_indexable_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            fixed_array_expression_length(program, facts, call.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            let length = expression_indexable_length(program, facts, indexed.collection)?;
            range_result_length(program, facts, indexed.index, length)
        }
        ExpressionNode::Member(member) => {
            facts.field_length(member.member_symbol, Some(member.member.as_str()))
        }
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        _ => None,
    }
}

fn folded_integer_binary(left: i64, operator: BinaryOperator, right: i64) -> Option<i64> {
    match operator {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Divide => (right != 0).then(|| left.checked_div(right)).flatten(),
        BinaryOperator::Modulo => (right != 0).then(|| left.checked_rem(right)).flatten(),
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::And
        | BinaryOperator::Equal
        | BinaryOperator::Greater
        | BinaryOperator::GreaterOrEqual
        | BinaryOperator::Less
        | BinaryOperator::LessOrEqual
        | BinaryOperator::NotEqual
        | BinaryOperator::Or
        | BinaryOperator::ShiftLeft
        | BinaryOperator::ShiftRight => None,
    }
}

fn range_result_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    length: usize,
) -> Option<usize> {
    let ExpressionNode::Range(range) = program.expression_table.expression(index) else {
        return None;
    };
    let (start, end) = provable_range_bounds(program, facts, range)?;
    let start = usize::try_from(start).ok()?;
    let end = end.map(usize::try_from).transpose().ok()?.unwrap_or(length);
    if start > end || end > length {
        return None;
    }
    Some(end.saturating_sub(start))
}

fn fixed_array_expression_length(
    program: &omega_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Member(member) => {
            facts.field_length(member.member_symbol, Some(member.member.as_str()))
        }
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        _ => None,
    }
}
