use psi_typed_trees::expression::{ExpressionHandle, ExpressionNode};

use super::super::facts::RangeFacts;
use super::integers::provable_range_bounds;

pub(in crate::checks::ranges) fn expression_indexable_length(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Call(call)
            if matches!(call.target.as_str(), "as_slice" | "as_mut_slice") =>
        {
            fixed_array_expression_length(program, facts, call.receiver)
        }
        ExpressionNode::Indexed(indexed) => {
            match expression_indexable_length(program, facts, indexed.collection) {
                // Known-length base: the subslice length is bounded by the base.
                Some(length) => range_result_length(program, facts, indexed.index, length),
                // Unknown-length base (e.g. a slice parameter): a window over
                // constant `a..b` bounds still has the derivable exact length
                // `b - a`, a first-class length fact for a shrunk window.
                None => fixed_range_window_length(program, facts, indexed.index),
            }
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
        ExpressionNode::Borrow(inner) => expression_indexable_length(program, facts, inner.target),
        _ => None,
    }
}

fn range_result_length(
    program: &psi_typed_trees::TypedTrees,
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

/// Derives the exact length of a window `[a..b]` taken over a base whose own
/// length is unknown. With both bounds constant-folded the window has exactly
/// `b - a` elements regardless of the base length (window-shrinking length
/// fact). An open-ended range (`a..`) has no derivable length without the base.
fn fixed_range_window_length(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
) -> Option<usize> {
    let ExpressionNode::Range(range) = program.expression_table.expression(index) else {
        return None;
    };
    let (start, end) = provable_range_bounds(program, facts, range)?;
    let end = end?;
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    if start > end {
        return None;
    }
    Some(end - start)
}

fn fixed_array_expression_length(
    program: &psi_typed_trees::TypedTrees,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            fixed_array_expression_length(program, facts, inner.target)
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
