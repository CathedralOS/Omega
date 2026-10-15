use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::machine::Machine;
use typed_trees::state::State;

use super::super::facts::RangeFacts;
use super::integers::provable_range_bounds;

pub(in crate::checks::ranges) fn expression_indexable_length(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    expression: ExpressionHandle,
) -> Option<usize> {
    if !expression.is_valid() {
        return None;
    }

    if let Some(length) = facts.expression_exact_length(program, machine, state, expression) {
        return usize::try_from(length).ok();
    }
    // Live descriptor geometry belongs to its canonical storage place, not
    // the spelling of a local name. The same loan evidence covers stored
    // fields and selected elements; it does not infer an extent from a type.
    if let Some(length) = facts
        .checked_calls
        .and_then(|calls| calls.collection_extent(program, facts.statement_index, expression))
    {
        return Some(length);
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::String(bytes) => Some(bytes.len()),
        ExpressionNode::Indexed(indexed) => {
            match expression_indexable_length(program, machine, state, facts, indexed.collection) {
                // Known-length base: the subslice length is bounded by the base.
                Some(length) => range_result_length(program, facts, indexed.index, length),
                // Unknown-length base (e.g. a slice parameter): a window over
                // constant `a..b` bounds still has the derivable exact length
                // `b - a`, a first-class length fact for a shrunk window.
                None => fixed_range_window_length(program, facts, indexed.index),
            }
        }
        ExpressionNode::Member(member) => facts.field_length(member.member_symbol).or_else(|| {
            super::super::types::expression_type_reference(program, machine, state, expression)
                .and_then(|type_reference| {
                    super::super::arrays::fixed_array_type_length(program, type_reference)
                })
        }),
        ExpressionNode::Name(path) => facts.local_length(
            path.symbol,
            program
                .expression_table
                .name_path_members(path.members)
                .last()
                .map(|name| name.as_str()),
        ),
        ExpressionNode::Borrow(inner) => {
            expression_indexable_length(program, machine, state, facts, inner.target)
        }
        _ => None,
    }
}

fn range_result_length(
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
    length: usize,
) -> Option<usize> {
    let range = program.expression_table.range_node(index)?;
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
    program: &dyn ::validation::CheckingInput,
    facts: &RangeFacts<'_>,
    index: ExpressionHandle,
) -> Option<usize> {
    let range = program.expression_table.range_node(index)?;
    let (start, end) = provable_range_bounds(program, facts, range)?;
    let end = end?;
    let start = usize::try_from(start).ok()?;
    let end = usize::try_from(end).ok()?;
    if start > end {
        return None;
    }
    Some(end - start)
}
