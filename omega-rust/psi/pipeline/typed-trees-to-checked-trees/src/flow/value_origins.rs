//! Capture exact value origins before local writes, not the last spelling of a slot.

use crate::flow::{self, CanonicalPlace};
use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::{
    TypedTrees, expression::ExpressionHandle, machine::Machine, statement::StatementNode,
    types::TypeReferenceHandle,
};

pub(crate) fn value_origin_at_call(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    let frames = validation::CallFrameResolver::new(program)?;
    place =
        flow::local_reference_storage_at_call(program, &frames, machine, flow, state, call, place)?;
    let typed_state = crate::semantic_calls::find_state(program, state.state_symbol)?;
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    for (index, statement) in statements
        .get(..call.statement_index)?
        .iter()
        .enumerate()
        .rev()
    {
        match statement {
            StatementNode::Assignment(assignment) => {
                if frames.assignment_replaces_local_reference_binding(machine, statement)? {
                    preserve_frame(
                        program,
                        machine,
                        state,
                        index,
                        &place,
                        &frames.statement_value_write_frame(machine, statement),
                    )?;
                    continue;
                }
                let target = flow::statement_mutated_place(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
                )?;
                let target = flow::local_reference_storage_before_statement(
                    program, &frames, machine, state, index, target,
                )?;
                if let Some(suffix) = exact_suffix(&place, &target) {
                    let stored_type = validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(typed_state),
                        assignment.target,
                    )?;
                    if !frames.proof_value_is_caller_isolated(stored_type) {
                        return None;
                    }
                    // The right-hand side is captured before this store. Keep
                    // tracing from that earlier point even if its slot changes
                    // again before the eventual qualified call.
                    let source = captured_source_place(
                        program,
                        state,
                        index,
                        assignment.value,
                        stored_type,
                        suffix,
                    )?;
                    place = flow::local_reference_storage_before_statement(
                        program, &frames, machine, state, index, source,
                    )?;
                } else {
                    let writes = flow::statement_storage_writes(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
                        Some(&frames),
                    )?;
                    if writes.iter().any(|write| overlaps(program, &place, write)) {
                        return None;
                    }
                }
            }
            StatementNode::LocalData(local) if place.root == PlaceRoot::Symbol(local.symbol) => {
                // References, including constrained or nested references, are
                // live aliases rather than captured copies of their referents.
                if !frames.proof_value_is_caller_isolated(local.type_reference) {
                    return None;
                }
                let source = captured_source_place(
                    program,
                    state,
                    index,
                    local.initial_value,
                    local.type_reference,
                    &place.segments,
                )?;
                place = flow::local_reference_storage_before_statement(
                    program, &frames, machine, state, index, source,
                )?;
            }
            StatementNode::Call(call) => {
                preserve_frame(
                    program,
                    machine,
                    state,
                    index,
                    &place,
                    &frames.may_write_frame(machine, call),
                )?;
            }
            _ => {}
        }
        preserve_frame(
            program,
            machine,
            state,
            index,
            &place,
            &frames.statement_value_write_frame(machine, statement),
        )?;
    }
    Some(place)
}

/// A constructor has no storage of its own: a projection into a captured
/// record or array literal arrives from the selected field or element
/// expression, so the origin continues from that operand with the remaining
/// projection. Any other expression keeps the whole projection.
fn captured_source_place(
    program: &TypedTrees,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    stored_type: TypeReferenceHandle,
    suffix: &[PlaceSegment],
) -> Option<CanonicalPlace> {
    let mut projections =
        flow::literal_value_projections(program, value, stored_type, suffix, false)?;
    let projection = projections.pop()?;
    if !projections.is_empty() {
        return None;
    }
    let mut source = flow::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        statement_index,
        projection.expression,
    )?;
    source.segments.extend_from_slice(&projection.remaining);
    Some(source)
}

fn exact_suffix<'place>(
    place: &'place CanonicalPlace,
    prefix: &CanonicalPlace,
) -> Option<&'place [PlaceSegment]> {
    (place.root == prefix.root && place.segments.starts_with(&prefix.segments))
        .then(|| &place.segments[prefix.segments.len()..])
}

fn overlaps(program: &TypedTrees, left: &CanonicalPlace, right: &CanonicalPlace) -> bool {
    flow::normalized_event_place_root(program, left.root)
        == flow::normalized_event_place_root(program, right.root)
        && flow::canonical_place_segments_may_overlap(program, &left.segments, &right.segments)
}

fn preserve_frame(
    program: &TypedTrees,
    machine: &Machine,
    state: &FlowStateFact,
    statement_index: usize,
    place: &CanonicalPlace,
    frame: &facts::NormalizedWriteFrame,
) -> Option<()> {
    let writes = flow::frame_storage_writes(
        program,
        machine.symbol,
        state.state_symbol,
        statement_index,
        frame,
    )?;
    (!writes.iter().any(|write| overlaps(program, place, write))).then_some(())
}
