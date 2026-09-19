//! Capture exact value origins before local writes, not the last spelling of a slot.

use crate::flow::{self, CanonicalPlace};
use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::{
    TypedTrees,
    expression::{ExpressionHandle, ExpressionNode, TableCallExpression},
    machine::Machine,
    statement::StatementNode,
    types::TypeReferenceHandle,
};

pub(crate) fn value_origin_at_call(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<CanonicalPlace> {
    value_origin_at_call_resolving(
        program,
        flow,
        machine,
        state,
        call,
        place,
        call_frames,
        |_, _, _, _| None,
        |_, _, _| None,
    )
}

/// The shared backward origin trace with two extra producers a domain may
/// prove: a decisive store whose captured value is an owned call result — the
/// resolver receives the result-position call, the store's statement index,
/// and the projection into the result value and returns the exact caller-side
/// place the result arrived from, or None to keep the call opaque — and a
/// demanded place that still names storage through a reference leaf carried
/// inside an owned local. The rebase receives the frontier bound (one past
/// the statement under examination, so it sees every store the frontier
/// already passed) and the demanded place; it returns the exact place the
/// demanded storage occupies, or None to keep the literal spelling for the
/// ordinary scan.
pub(crate) fn value_origin_at_call_resolving<Resolve, Rebase>(
    program: &TypedTrees,
    flow: &FlowFacts,
    machine: &Machine,
    state: &FlowStateFact,
    call: &FlowCallFact,
    mut place: CanonicalPlace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    resolve: Resolve,
    rebase: Rebase,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
    Rebase: Fn(&FlowStateFact, usize, &CanonicalPlace) -> Option<CanonicalPlace>,
{
    let mut owned_frames = None;
    let frames = flow::shared_call_frames_or(call_frames, program, &mut owned_frames)?;
    place =
        flow::local_reference_storage_at_call(program, frames, machine, flow, state, call, place)?;
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
        // The demanded place may still name storage through a reference leaf
        // an ordinary arm cannot resolve — `boxed.view.scheduler` where `view`
        // is a `&` slot inside an owned local. Rebase it to the referent the
        // leaf holds at this frontier before the statement replays; a leaf
        // slot the frontier already passed was either resolved by this rebase
        // or refused by the overlap checks, so `index + 1` is the exact
        // prefix the leaf's contents came from.
        if let Some(rebased) = rebase(state, index + 1, &place) {
            place = rebased;
        }
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
                        frames,
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
                    program, frames, machine, state, index, target,
                )?;
                // A write spelled through a write-capable leaf or a mutable
                // reference binding still lands in the referent's storage.
                // Match the demanded place against both spellings so the
                // exact store can be captured whichever spelling it carries.
                let storage_target = flow::rebase_exact_local_place(
                    program,
                    state.state_symbol,
                    index,
                    target.clone(),
                    Some(frames),
                )
                .filter(|storage_target| *storage_target != target);
                let suffix = exact_suffix(&place, &target).or_else(|| {
                    storage_target
                        .as_ref()
                        .and_then(|storage_target| exact_suffix(&place, storage_target))
                });
                if let Some(suffix) = suffix {
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
                        &resolve,
                    )?;
                    place = reference_storage_or_literal(
                        program, frames, machine, state, index, source,
                    );
                } else {
                    let writes = flow::statement_storage_writes(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
                        Some(frames),
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
                    &resolve,
                )?;
                place =
                    reference_storage_or_literal(program, frames, machine, state, index, source);
            }
            StatementNode::Call(call) => {
                preserve_frame(
                    program,
                    machine,
                    state,
                    index,
                    &place,
                    &frames.may_write_frame(machine, call),
                    frames,
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
            frames,
        )?;
    }
    Some(place)
}

/// A constructor has no storage of its own: a projection into a captured
/// record or array literal arrives from the selected field or element
/// expression, so the origin continues from that operand with the remaining
/// projection. A result-position call asks the domain resolver for the exact
/// caller place its checked callee proves; an unresolved call stays opaque.
/// Any other expression keeps the whole projection.
fn captured_source_place<Resolve>(
    program: &TypedTrees,
    state: &FlowStateFact,
    statement_index: usize,
    value: ExpressionHandle,
    stored_type: TypeReferenceHandle,
    suffix: &[PlaceSegment],
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let mut projections =
        flow::literal_value_projections(program, value, stored_type, suffix, false)?;
    let projection = projections.pop()?;
    if !projections.is_empty() {
        return None;
    }
    if let Some((call, relative)) =
        result_call(program, projection.expression, &projection.remaining)
        && let Some(place) = resolve(state, statement_index, call, &relative)
    {
        return Some(place);
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

/// Peel member and index projections around a result-position call,
/// accumulating the path into the result value in place-segment order. The
/// result-relative path is the peeled projection followed by the subject's
/// remaining projection.
fn result_call<'program>(
    program: &'program TypedTrees,
    mut expression: ExpressionHandle,
    remaining: &[PlaceSegment],
) -> Option<(&'program TableCallExpression, Vec<PlaceSegment>)> {
    let mut relative = Vec::new();
    loop {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                let mut peeled = Vec::new();
                flow::push_field_place_segments(
                    program,
                    &mut peeled,
                    flow::effective_member_symbol(program, member.receiver, member),
                );
                peeled.extend_from_slice(&relative);
                relative = peeled;
                expression = member.receiver;
            }
            ExpressionNode::Indexed(indexed) => {
                relative.insert(0, flow::index_place_segment(program, indexed.index));
                expression = indexed.collection;
            }
            ExpressionNode::Call(call) => {
                relative.extend_from_slice(remaining);
                return Some((call, relative));
            }
            _ => return None,
        }
    }
}

/// Rebase a captured source through the prefix's exact reference origins
/// when they name it. A source rooted at a reference local whose origin the
/// shared query cannot prove — an exclusive `&mut` binding or a leaf carried
/// inside another local — keeps its literal spelling instead of ending the
/// trace: each later frontier's rebase either resolves that storage exactly
/// or the scan still meets the same write fences and the declaration's
/// caller-isolation check. For consumers without a rebase hook the literal
/// spelling still fails closed at the first reference-typed declaration.
fn reference_storage_or_literal(
    program: &TypedTrees,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    source: CanonicalPlace,
) -> CanonicalPlace {
    flow::local_reference_storage_before_statement(
        program,
        frames,
        machine,
        state,
        index,
        source.clone(),
    )
    .unwrap_or(source)
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
    call_frames: &validation::CallFrameResolver<'_>,
) -> Option<()> {
    let writes = flow::frame_storage_writes(
        program,
        machine.symbol,
        state.state_symbol,
        statement_index,
        frame,
        Some(call_frames),
    )?;
    (!writes.iter().any(|write| overlaps(program, place, write))).then_some(())
}
