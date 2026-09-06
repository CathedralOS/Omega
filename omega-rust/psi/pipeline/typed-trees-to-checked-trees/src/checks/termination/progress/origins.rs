//! Capture exact value origins before local writes, not the last spelling of a slot.

use super::{FlowCallFact, FlowFacts, FlowStateFact, ProgressSubject};
use crate::flow::{self, CanonicalPlace};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::{TypedTrees, machine::Machine, statement::StatementNode};

#[cfg(test)]
mod tests;

pub(super) fn at_call(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    subject: ProgressSubject,
) -> Option<ProgressSubject> {
    let frames = validation::CallFrameResolver::new(program)?;
    let mut place = CanonicalPlace {
        root: PlaceRoot::Symbol(subject.root),
        segments: Vec::new(),
    };
    for projection in subject.projections {
        flow::push_field_place_segments(program, &mut place.segments, projection);
    }
    // Call rows are retained in execution order, not authored preorder ordinal.
    // Earlier operand effects must not be silently borrowed as entry identity.
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
            crate::CallSite::Statement(call) => frames.may_write_frame(machine, call),
            crate::CallSite::Expression { expression, .. } => {
                frames.expression_write_frame(machine, expression)
            }
            crate::CallSite::TransitionNamed { .. } => return None,
        };
        preserve_frame(
            program,
            machine,
            state,
            call.statement_index,
            &place,
            &frame,
        )?;
    }
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
                let target = flow::statement_mutated_place(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
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
                    // again before the eventual progress-dependent call.
                    let mut source = flow::canonical_place_from_expression_in_state(
                        program,
                        state.state_symbol,
                        index,
                        assignment.value,
                    )?;
                    source.segments.extend_from_slice(suffix);
                    place = source;
                } else {
                    let writes = flow::statement_storage_writes(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
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
                let mut source = flow::canonical_place_from_expression_in_state(
                    program,
                    state.state_symbol,
                    index,
                    local.initial_value,
                )?;
                source.segments.extend_from_slice(&place.segments);
                place = source;
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
    super::subject_from_place(place.root, &place.segments)
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
