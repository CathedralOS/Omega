//! Rebase a live reference before tracing the value currently in its referent.

use super::{
    CanonicalPlace, canonical_place_segments_may_overlap, frame_storage_writes,
    normalized_event_place_root,
};
use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use facts::PlaceRoot;
use typed_trees::types::TypeReferenceNode;
use typed_trees::{TypedTrees, machine::Machine, statement::StatementNode};

#[cfg(test)]
mod tests;

pub(crate) fn local_reference_storage_before_statement(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    let PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut declarations =
        statements
            .get(..index)?
            .iter()
            .filter_map(|statement| match statement {
                StatementNode::LocalData(local) if local.symbol == root => Some(local),
                _ => None,
            });
    let Some(local) = declarations.next() else {
        return Some(place);
    };
    if declarations.next().is_some() {
        return None;
    }
    let mut reference = local.type_reference;
    while let TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(reference)
    {
        reference = *base_type;
    }
    if !matches!(
        program.type_reference_table.type_reference(reference),
        TypeReferenceNode::Reference { .. }
    ) {
        return Some(place);
    }
    let (root, mut segments) = frames.local_reference_origin_before_statement(
        machine,
        statements.get(index)?,
        local.symbol,
    )?;
    segments.extend_from_slice(&place.segments);
    place.root = PlaceRoot::Symbol(root);
    place.segments = segments;
    Some(place)
}

pub(crate) fn local_reference_storage_at_call(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    let resolved = local_reference_storage_before_statement(
        program,
        frames,
        machine,
        state,
        call.statement_index,
        place.clone(),
    )?;
    preserve_call_prefix_storage(
        program,
        frames,
        machine,
        flow,
        state,
        call,
        &[place, resolved.clone()],
    )?;
    Some(resolved)
}

/// Earlier operands may replace either the reference binding or its referent.
/// The statement-prefix origin alone says nothing about those effects.
/// Calls are retained in execution order, not authored preorder ordinal.
fn preserve_call_prefix_storage(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    places: &[CanonicalPlace],
) -> Option<()> {
    let calls = flow.control.calls.span_or_empty(state.calls);
    let position = calls
        .iter()
        .position(|candidate| std::ptr::eq(candidate, call))?;
    for prior in calls[..position]
        .iter()
        .filter(|prior| prior.statement_index == call.statement_index)
    {
        let site = crate::find_call_site(
            program,
            machine.symbol,
            state.state_symbol,
            prior.statement_index,
            prior.call_ordinal,
        )?;
        let frame = match site {
            crate::CallSite::Statement(call) => {
                if !frames.call_reference_bindings_are_stable(machine, call) {
                    return None;
                }
                frames.may_write_frame(machine, call)
            }
            crate::CallSite::Expression { expression, .. } => {
                if !frames.expression_reference_bindings_are_stable(machine, expression) {
                    return None;
                }
                frames.expression_write_frame(machine, expression)
            }
            crate::CallSite::TransitionNamed { .. } => return None,
        };
        preserve_frame(
            program,
            machine,
            state,
            call.statement_index,
            places,
            &frame,
        )?;
    }
    Some(())
}

fn preserve_frame(
    program: &TypedTrees,
    machine: &Machine,
    state: &FlowStateFact,
    statement_index: usize,
    places: &[CanonicalPlace],
    frame: &facts::NormalizedWriteFrame,
) -> Option<()> {
    let writes = frame_storage_writes(
        program,
        machine.symbol,
        state.state_symbol,
        statement_index,
        frame,
    )?;
    if places.iter().any(|place| {
        writes.iter().any(|write| {
            normalized_event_place_root(program, place.root)
                == normalized_event_place_root(program, write.root)
                && canonical_place_segments_may_overlap(program, &place.segments, &write.segments)
        })
    }) {
        return None;
    }
    Some(())
}
