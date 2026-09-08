//! Whole replacement changes live extent; builtin byte replacement does not.

use typed_trees::expression::{ExpressionHandle, ExpressionNode};
use typed_trees::{TypedTrees, machine::Machine, state::State};

use super::{
    arrays::bounded_byte_type_capacity, expressions::expression_indexable_length,
    facts::RangeFacts, types::expression_type_reference,
};

pub(super) fn replacement_length(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<usize> {
    expression_type_reference(program, machine, state, target)
        .and_then(|reference| super::arrays::fixed_array_type_length(program, reference))
        .or_else(|| expression_indexable_length(program, machine, state, facts, value))
}

pub(super) fn value_preserves_indexed_extent<'program>(
    program: &'program TypedTrees,
    machine: &'program Machine,
    state: &State,
    call_frames: Option<&validation::CallFrameResolver<'program>>,
    facts: &mut RangeFacts<'_>,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> bool {
    let ExpressionNode::Indexed(indexed) = program.expression_table.expression(target) else {
        return true;
    };
    if expression_type_reference(program, machine, state, indexed.collection)
        .and_then(|reference| bounded_byte_type_capacity(program, reference))
        .is_none()
    {
        return true;
    }
    let Some(paths) = call_frames.and_then(|frames| {
        frames
            .expression_write_frame(machine, value)
            .into_complete_paths()
    }) else {
        return false;
    };
    if paths.is_empty() {
        return true;
    }
    // The target and its selectors were evaluated before the RHS. Capture
    // those typed dependencies now, rather than resolving the target again
    // against values a nested RHS call may have replaced.
    facts.record_expression_dependencies(program, machine, state, indexed.collection);
    let writes = crate::flow::frame_storage_writes(
        program,
        machine.symbol,
        state.symbol,
        facts.statement_index,
        &facts::NormalizedWriteFrame::complete(paths),
    );
    facts.expression_is_disjoint_from_writes(
        program,
        machine,
        state,
        indexed.collection,
        writes.as_deref(),
    )
}

pub(super) fn assigned_extent(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &RangeFacts<'_>,
    target: ExpressionHandle,
    value: ExpressionHandle,
) -> Option<(ExpressionHandle, usize)> {
    let (collection, source) = match program.expression_table.expression(target) {
        ExpressionNode::Indexed(indexed)
            if super::indexes::is_builtin_scalar_index(program, machine, state, facts, target) =>
        {
            (indexed.collection, indexed.collection)
        }
        ExpressionNode::Name(_) | ExpressionNode::Member(_) => (target, value),
        _ => return None,
    };
    bounded_byte_type_capacity(
        program,
        expression_type_reference(program, machine, state, collection)?,
    )?;
    Some((
        collection,
        expression_indexable_length(program, machine, state, facts, source)?,
    ))
}

pub(super) fn seed_assigned_extent(
    program: &TypedTrees,
    machine: &Machine,
    state: &State,
    facts: &mut RangeFacts<'_>,
    extent: Option<(ExpressionHandle, usize)>,
) {
    let Some((collection, length)) = extent else {
        return;
    };
    let Ok(length) = i64::try_from(length) else {
        return;
    };
    let label = program.expression_table.display_name(collection);
    // Assignment retires content/selector facts even when just its length is
    // unchanged. Never min-merge the previous value's extent with a new value.
    facts.forget_collection_expression(&label);
    facts.record_expression_dependencies(program, machine, state, collection);
    facts.prove_exact_length(label, length);
}
