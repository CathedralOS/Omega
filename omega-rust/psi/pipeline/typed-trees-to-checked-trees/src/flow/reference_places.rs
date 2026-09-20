//! Rebase a live reference before tracing the value currently in its referent.

use super::{
    CanonicalPlace, canonical_place_segments_may_overlap, frame_storage_writes,
    normalized_event_place_root,
};
use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use facts::PlaceRoot;
use typed_trees::types::TypeReferenceNode;
use typed_trees::{TypedTrees, machine::Machine, statement::StatementNode};

mod result_candidates;
#[cfg(test)]
mod tests;

pub(crate) use result_candidates::{
    call_result_sources, reference_expression_storage_places,
    reference_result_candidates_before_statement,
};

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

/// Every storage place a call actual names through a reference-result local:
/// the finite candidate origins (`level.rooms[0]` .. `level.rooms[15]` for
/// `level.room_mut(cell)`) with the actual's own projection applied, so
/// `room.event` arrives as `level.rooms[i].event`. A caller-side proof over
/// such an actual holds exactly when every candidate proves it -- the callee
/// receives whichever storage the loan names. `None` keeps the single-origin
/// or conservative treatment, including when an earlier operand's frame could
/// have written a candidate.
pub(crate) fn local_reference_candidate_storages_at_call(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    borrow: &checked_trees::BorrowFacts,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let PlaceRoot::Symbol(root) = place.root else {
        return None;
    };
    let candidates = reference_result_candidates_before_statement(
        program,
        state.state_symbol,
        call.statement_index,
        root,
        Some(frames),
    )?;
    let mut storages: Vec<CanonicalPlace> = candidates
        .into_iter()
        .map(|mut candidate| {
            candidate.segments.extend_from_slice(&place.segments);
            candidate
        })
        .collect();
    storages.push(place);
    preserve_candidate_call_prefix_storage(
        program, frames, borrow, machine, flow, state, call, &storages,
    )?;
    storages.pop();
    Some(storages)
}

/// The prefix bound is the call's own position in the state's recorded
/// execution order. Resolve it from the row's recorded identity — the
/// `(statement_index, call_ordinal)` coordinate plus target and receiver
/// symbols, the same fields `build_call_entry_contexts` uses — never the
/// address the row occupies in this arena slice. A replayed copy of the
/// same recorded row reaches the same bound; an absent or ambiguous
/// identity declines rather than borrowing a same-shaped row's position.
fn call_position_in_state(
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
) -> Option<usize> {
    let calls = flow.control.calls.span_or_empty(state.calls);
    let mut positions = calls
        .iter()
        .enumerate()
        .filter_map(|(position, candidate)| {
            (candidate.statement_index == call.statement_index
                && candidate.call_ordinal == call.call_ordinal
                && candidate.target_symbol == call.target_symbol
                && candidate.receiver_symbol == call.receiver_symbol)
                .then_some(position)
        });
    let position = positions.next()?;
    if positions.next().is_some() {
        return None;
    }
    Some(position)
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
    let position = call_position_in_state(flow, state, call)?;
    let calls = flow.control.calls.span_or_empty(state.calls);
    for prior in calls[..position]
        .iter()
        .filter(|prior| prior.statement_index == call.statement_index)
    {
        let site = crate::semantic_calls::find_call_site(
            program,
            machine.symbol,
            state.state_symbol,
            prior.statement_index,
            prior.call_ordinal,
        )?;
        let frame = match site {
            crate::semantic_calls::CallSite::Statement(call) => {
                if !frames.call_reference_bindings_are_stable(machine, call) {
                    return None;
                }
                frames.may_write_frame(machine, call)
            }
            crate::semantic_calls::CallSite::Expression { expression, .. } => {
                if !frames.expression_reference_bindings_are_stable(machine, expression) {
                    return None;
                }
                frames.expression_write_frame(machine, expression)
            }
            crate::semantic_calls::CallSite::TransitionNamed { .. } => return None,
        };
        preserve_frame(
            program,
            machine,
            state,
            call.statement_index,
            places,
            &frame,
            frames,
        )?;
    }
    Some(())
}

/// The prefix gate for a candidate-bearing local asks the question flow's own
/// invalidation already answered for every earlier operand call: which caller
/// storage could the call reach? `signature_ceiling_places` names the declared
/// ceiling of the prior call -- its exclusive actuals and `&mut` receiver,
/// rebased through the same divergent-alias candidate map used here -- so a
/// nested operand that cannot reach the traced slot or any candidate referent
/// preserves the whole set. The resolver's reference-binding walk deliberately
/// declines once a divergent binding exists in the prefix, so that gate cannot
/// serve this path; the ceiling stays sound because every caller-visible write
/// goes through an exclusive borrow it names.
fn preserve_candidate_call_prefix_storage(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    borrow: &checked_trees::BorrowFacts,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    places: &[CanonicalPlace],
) -> Option<()> {
    let position = call_position_in_state(flow, state, call)?;
    let calls = flow.control.calls.span_or_empty(state.calls);
    let borrow_calls = borrow
        .states
        .iter()
        .find_map(|(_, owner)| {
            (owner.machine_symbol == machine.symbol && owner.state_symbol == state.state_symbol)
                .then(|| borrow.calls.span_or_empty(owner.calls))
        })
        .unwrap_or(&[]);
    for prior in calls[..position]
        .iter()
        .filter(|prior| prior.statement_index == call.statement_index)
    {
        let borrow_call = borrow_calls.iter().find(|candidate| {
            candidate.statement_index == prior.statement_index
                && candidate.call_ordinal == prior.call_ordinal
                && candidate.target_symbol == prior.target_symbol
        })?;
        let writes = crate::flow::signature_ceiling_places(
            program,
            machine.symbol,
            state.state_symbol,
            borrow_call,
            Some(frames),
        )?;
        if places.iter().any(|place| {
            writes.iter().any(|write| {
                normalized_event_place_root(program, place.root)
                    == normalized_event_place_root(program, write.root)
                    && canonical_place_segments_may_overlap(
                        program,
                        &place.segments,
                        &write.segments,
                    )
            })
        }) {
            return None;
        }
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
    call_frames: &validation::CallFrameResolver<'_>,
) -> Option<()> {
    let writes = frame_storage_writes(
        program,
        machine.symbol,
        state.state_symbol,
        statement_index,
        frame,
        Some(call_frames),
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
