//! leaf arrivals slice of origins.
use validation::CanonicalPlace;

use super::reference_bounds::*;

use super::FlowStateFact;
use crate::flow::{self};
use facts::{PlaceRoot, PlaceSegment};
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::machine::Machine;
use typed_trees::statement::StatementNode;
use typed_trees::types::{TypeReferenceHandle, TypeReferenceNode};

/// The declared type of the `LocalData` declaring `root` before `bound`, when
/// `root` is a local at all — parameters and members have no declaration
/// statement.
pub(super) fn statements_local_type(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    root: symbols::SymbolHandle,
    bound: usize,
) -> Option<TypeReferenceHandle> {
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    program
        .state_local_declaration(typed_state, root)
        .filter(|&(ordinal, _)| ordinal < bound)
        .map(|(_, local)| local.type_reference)
}

/// What one resolved store did to the leaf slot under the scan.
pub(super) enum LeafArrival {
    /// The slot's contents trace to this exact storage — the referent for a
    /// shared leaf, or the caller-side place a proven call result carried.
    Resolved(CanonicalPlace),
    /// The slot's contents were copied out of another shared-reference slot;
    /// the scan continues with that slot's earlier writes at the same point
    /// in time.
    Relocate(CanonicalPlace),
}

/// Resolve the exact referent a shared-reference leaf inside an owned local
/// holds at `call`, when the demanded place crosses one. `place` names the
/// demanded storage; its leading `leaf` segments select a `&`-typed field of
/// a local carrier — `boxed.view` inside `boxed.view.scheduler`. Nothing can
/// write through a shared reference, so the only writes that matter are the
/// ones rebinding the slot itself or replacing an enclosing slot: each is the
/// store that supplied the referent. The scan walks backward to the latest
/// such store — an assignment target equal to the slot (rebind), an ancestor
/// of it (carrier replacement), or the local's own declaration — and resolves
/// the stored operand's referent. Every other overlapping write — a call
/// frame, an operand call embedded in any statement, a through-leaf or
/// alias-closed store — fails closed. A leaf value copied out of another
/// carrier's leaf relocates the slot and keeps scanning that slot's earlier
/// writes, so the referent is the one held at the copy, not the one the
/// source slot holds at the call.
pub(super) fn shared_reference_leaf_origin<Resolve>(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    bound: usize,
    place: &CanonicalPlace,
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    let statements = program
        .statement_table
        .statements(typed_state.statement_nodes);
    let root = place.root.as_symbol()?;
    // Only an owned local's declaration supplies the boundary's frame type; a
    // parameter or member root keeps its access path unchanged.
    let (_, local) = program
        .state_local_declaration(typed_state, root)
        .filter(|&(ordinal, _)| ordinal < bound)?;
    let leaf_len = shared_reference_boundary(program, local.type_reference, &place.segments)?;
    let mut slot = CanonicalPlace {
        root: place.root,
        segments: place.segments[..leaf_len].to_vec(),
    };
    let suffix = &place.segments[leaf_len..];
    for (index, statement) in statements.get(..bound)?.iter().enumerate().rev() {
        // Operand calls embedded in any statement — including the supplying
        // store itself — run first; a frame that may touch the slot keeps the
        // leaf's contents unproven.
        if statement_may_touch_slot(program, machine, state, index, frames, statement, &slot)? {
            return None;
        }
        let arrival = match statement {
            StatementNode::LocalData(local) if slot.root == PlaceRoot::Symbol(local.symbol) => {
                // The carrier's own declaration supplied the leaf: the
                // initializer's operand at the leaf path is the stored value.
                let candidate = leaf_value_candidate(
                    program,
                    state,
                    index,
                    local.initial_value,
                    local.type_reference,
                    &slot.segments,
                    resolve,
                )?;
                Some(leaf_candidate_arrival(
                    program, frames, machine, state, index, candidate,
                )?)
            }
            StatementNode::Assignment(assignment) => {
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
                let target_is_slot = target.root == slot.root;
                let stored_type = || {
                    validation::declared_place_type_raw(
                        program,
                        machine,
                        Some(typed_state),
                        assignment.target,
                    )
                };
                if target_is_slot && target.segments == slot.segments {
                    // A store into exactly this slot rebinds the leaf: the
                    // right-hand value is the new referent.
                    let candidate = leaf_value_candidate(
                        program,
                        state,
                        index,
                        assignment.value,
                        stored_type()?,
                        &[],
                        resolve,
                    )?;
                    Some(leaf_candidate_arrival(
                        program, frames, machine, state, index, candidate,
                    )?)
                } else if target_is_slot && slot.segments.starts_with(&target.segments) {
                    // A store into an enclosing slot replaces the carrier; the
                    // leaf's contents are the projection of the stored value
                    // at the remaining leaf path.
                    let candidate = leaf_value_candidate(
                        program,
                        state,
                        index,
                        assignment.value,
                        stored_type()?,
                        &slot.segments[target.segments.len()..],
                        resolve,
                    )?;
                    Some(leaf_candidate_arrival(
                        program, frames, machine, state, index, candidate,
                    )?)
                } else {
                    if !statement_writes_miss_slot(
                        program,
                        machine.symbol,
                        state.state_symbol,
                        index,
                        statement,
                        frames,
                        &slot,
                    )? {
                        return None;
                    }
                    None
                }
            }
            StatementNode::Call(call_statement) => {
                let writes = flow::frame_storage_writes(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    &frames.may_write_frame(machine, call_statement),
                    Some(frames),
                )?;
                if writes
                    .iter()
                    .any(|write| places_may_overlap(program, &slot, write))
                {
                    return None;
                }
                None
            }
            _ => {
                if !statement_writes_miss_slot(
                    program,
                    machine.symbol,
                    state.state_symbol,
                    index,
                    statement,
                    frames,
                    &slot,
                )? {
                    return None;
                }
                None
            }
        };
        match arrival {
            Some(LeafArrival::Resolved(mut resolved)) => {
                resolved.segments.extend_from_slice(suffix);
                return Some(resolved);
            }
            Some(LeafArrival::Relocate(next)) => slot = next,
            None => {}
        }
    }
    // The prefix never stored the slot: it names its root's entry value. A
    // parameter or member root keeps the access path, and the delegated scan
    // still proves or refuses it under its ordinary rules — including any
    // later rebind the relocated slot endured before the call.
    let mut resolved = slot;
    resolved.segments.extend_from_slice(suffix);
    Some(resolved)
}

/// The first `&`-shared boundary in a demanded path: the number of leading
/// segments whose projection through the root's declared type lands on a
/// shared reference. An exclusive boundary is not this scan's subject —
/// write-capable leaves rebase through the prefix's stored origins — and an
/// unresolvable projection leaves the literal place for the delegated scan.
pub(super) fn shared_reference_boundary(
    program: &dyn ::validation::CheckingInput,
    root_type: TypeReferenceHandle,
    segments: &[PlaceSegment],
) -> Option<usize> {
    for boundary in 1..=segments.len() {
        let reached = validation::project_type_reference_from_segments(
            program,
            root_type,
            &segments[..boundary],
        )?;
        if let Some(access) = reference_type(program, reached) {
            return (!access.is_exclusive()).then_some(boundary);
        }
    }
    None
}

/// The operand that supplied a stored leaf: `wanted` projects the stored
/// `value` — a declaration initializer or an assignment's right-hand side —
/// down to the expression bound for the leaf, which canonicalizes to a place.
/// A result-position call instead asks the domain resolver for the caller
/// place its checked callee proves; every other non-storage expression —
/// conditional routes, opaque projections — is refused rather than borrowed
/// as a same-shaped guess.
pub(super) fn leaf_value_candidate<Resolve>(
    program: &dyn ::validation::CheckingInput,
    state: &FlowStateFact,
    index: usize,
    value: ExpressionHandle,
    declared_type: TypeReferenceHandle,
    wanted: &[PlaceSegment],
    resolve: &Resolve,
) -> Option<CanonicalPlace>
where
    Resolve:
        Fn(&FlowStateFact, usize, &TableCallExpression, &[PlaceSegment]) -> Option<CanonicalPlace>,
{
    let mut projections =
        flow::literal_expression_projections(program, value, declared_type, wanted, false)?;
    if projections.len() != 1 {
        return None;
    }
    let projection = projections.remove(0);
    let mut candidate = validation::canonical_place_from_expression_in_state(
        program,
        state.state_symbol,
        index,
        projection.expression,
    )?;
    candidate.segments.extend_from_slice(&projection.remaining);
    match candidate.root {
        PlaceRoot::Symbol(_) => Some(candidate),
        PlaceRoot::Expression(rooted) => match program.expression_table.expression(rooted) {
            ExpressionNode::Call(call) => resolve(state, index, call, &candidate.segments),
            _ => None,
        },
        _ => None,
    }
}

/// Classify the place a stored leaf operand arrived at. A candidate whose
/// root names no local in this prefix — a parameter or machine member — keeps
/// its access path. A reference-typed local names its referent rather than
/// its slot, so it rebases through the reference query at this exact prefix
/// position and classifies again. An owned local whose projected path still
/// lands on a shared reference is another carrier's leaf slot: the scan
/// relocates to it. Anything else is the referent itself.
pub(super) fn leaf_candidate_arrival(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    mut candidate: CanonicalPlace,
) -> Option<LeafArrival> {
    let typed_state = program.state_by_symbol(state.state_symbol)?;
    for _ in 0..REFERENCE_BOUNDARY_HOPS {
        let root = candidate.root.as_symbol()?;
        let Some((_, local)) = program
            .state_local_declaration(typed_state, root)
            .filter(|&(ordinal, _)| ordinal < index)
        else {
            return Some(LeafArrival::Resolved(candidate));
        };
        if reference_type(program, local.type_reference).is_some() {
            // The candidate names storage through this binding, not the
            // binding itself: rebase to its referent at this prefix position,
            // then classify that place.
            candidate = flow::local_reference_storage_before_statement(
                program, frames, machine, state, index, candidate,
            )?;
            continue;
        }
        let reached = validation::project_type_reference_from_segments(
            program,
            local.type_reference,
            &candidate.segments,
        )?;
        return Some(match reference_type(program, reached) {
            Some(language_semantics::ReferenceAccess::Shared) => LeafArrival::Relocate(candidate),
            _ => LeafArrival::Resolved(candidate),
        });
    }
    None
}

/// The reference access a type reference names after peeling `Constrained`
/// shells, or none when it names no reference at all.
pub(super) fn reference_type(
    program: &dyn ::validation::CheckingInput,
    mut reference: TypeReferenceHandle,
) -> Option<language_semantics::ReferenceAccess> {
    loop {
        match program.type_reference_table.type_reference(reference) {
            TypeReferenceNode::Constrained { base_type, .. } => reference = *base_type,
            TypeReferenceNode::Reference { access, .. } => return Some(*access),
            _ => return None,
        }
    }
}

/// Whether `statement`'s own write frame may touch `slot` — operand calls
/// embedded in any statement run first, so the frame's writes decide.
pub(super) fn statement_may_touch_slot(
    program: &dyn ::validation::CheckingInput,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    frames: &validation::CallFrameResolver<'_>,
    statement: &StatementNode,
    slot: &CanonicalPlace,
) -> Option<bool> {
    let writes = flow::frame_storage_writes(
        program,
        machine.symbol,
        state.state_symbol,
        index,
        &frames.statement_value_write_frame(machine, statement),
        Some(frames),
    )?;
    Some(
        writes
            .iter()
            .any(|write| places_may_overlap(program, slot, write)),
    )
}
