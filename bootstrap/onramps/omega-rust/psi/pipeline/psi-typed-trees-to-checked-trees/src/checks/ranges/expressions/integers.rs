use psi_symbols::SymbolHandle;
use psi_typed_trees::expression::{
    BinaryOperator, ExpressionHandle, ExpressionNode, TableRangeExpression,
};

use super::super::facts::RangeFacts;

/// Resolves the lower bound and the *normalized exclusive* upper bound of a
/// range to constant integers, when provable.
///
/// An inclusive range `a..=b` is normalized to the exclusive range `a..(b + 1)`
/// per the frozen Wave 0 range spec. The `b + 1` normalization uses
/// `checked_add`, so an inclusive range whose end is `i64::MAX` (e.g. the
/// `..=usize::MAX` overflow edge) reports a proof failure (`None`) rather than
/// silently wrapping — the overflow is surfaced as a diagnostic, never a panic.
pub(in crate::checks::ranges) fn provable_range_bounds(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    range: &TableRangeExpression,
) -> Option<(i64, Option<i64>)> {
    let start = if range.start.is_valid() {
        expression_integer_value(program, facts, range.start)?
    } else {
        0
    };
    let end = if range.end.is_valid() {
        let end = expression_integer_value(program, facts, range.end)?;
        Some(normalize_exclusive_end(end, range.end_inclusive)?)
    } else {
        None
    };
    Some((start, end))
}

/// Normalizes an inclusive end bound `b` to the exclusive end `b + 1`.
///
/// Returns `None` on overflow so the caller can report a proof error instead of
/// wrapping (the `..=` overflow edge is a proof failure, never a panic).
pub(in crate::checks::ranges) fn normalize_exclusive_end(
    end: i64,
    end_inclusive: bool,
) -> Option<i64> {
    if end_inclusive {
        end.checked_add(1)
    } else {
        Some(end)
    }
}

pub(in crate::checks::ranges) fn expression_integer_value(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<i64> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Binary(binary) => {
            let left = expression_integer_value(program, facts, binary.left)?;
            let right = expression_integer_value(program, facts, binary.right)?;
            folded_integer_binary(left, binary.operator, right)
        }
        ExpressionNode::Integer(value) => value.value_i64(),
        ExpressionNode::Name(_) => {
            let (symbol, name) = expression_name(program, expression)?;
            facts.local_integer(symbol, name)
        }
        ExpressionNode::Member(member) => {
            facts.field_integer(member.member_symbol, Some(member.member.as_str()))
        }
        _ => None,
    }
}

pub(in crate::checks::ranges) fn expression_name(
    program: &psi_typed_trees::TypedTrees,
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

pub(in crate::checks::ranges) fn folded_integer_binary(
    left: i64,
    operator: BinaryOperator,
    right: i64,
) -> Option<i64> {
    match operator {
        BinaryOperator::Add => left.checked_add(right),
        BinaryOperator::Divide => (right != 0).then(|| left.checked_div(right)).flatten(),
        BinaryOperator::Modulo => (right != 0).then(|| left.checked_rem(right)).flatten(),
        BinaryOperator::Multiply => left.checked_mul(right),
        BinaryOperator::Subtract => left.checked_sub(right),
        BinaryOperator::And
        | BinaryOperator::BitwiseAnd
        | BinaryOperator::BitwiseOr
        | BinaryOperator::BitwiseXor
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
