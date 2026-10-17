//! Rebase a live reference before tracing the value currently in its referent.

use validation::CanonicalPlace;

use super::canonical_place_segments_may_overlap;
use checked_trees::{FlowCallFact, FlowFacts, FlowStateFact};
use facts::PlaceRoot;
use typed_trees::types::TypeReferenceNode;
use typed_trees::{machine::Machine, statement::StatementNode};

mod result_candidates;

pub(crate) use result_candidates::{
    reference_expression_storage_places, reference_expression_storage_places_including_shared,
    reference_result_candidates_before_statement,
};

/// The expression whose place a reference-typed local currently refers to:
/// its initializer, or the value of the latest whole-local reassignment.
/// Returns `None` when the symbol is not a reference-typed local or the route
/// cannot be replayed.
pub(crate) fn reference_route_source(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
    statements: &[StatementNode],
    statement_index: usize,
    symbol: symbols::SymbolHandle,
) -> Option<typed_trees::expression::ExpressionHandle> {
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order
        .position(statement_index)
        .or_else(|| (statement_index == statements.len()).then_some(order.indices().len()))?;
    let (ordinal, declaration) = program.state_local_declaration(state, symbol)?;
    if !order
        .position(ordinal)
        .is_some_and(|position| position < end)
    {
        return None;
    }
    if !crate::checks::type_reference_is_reference(program, declaration.type_reference) {
        return None;
    }
    order.indices()[..end]
        .iter()
        .rev()
        .filter_map(|index| statements.get(*index))
        .find_map(|statement| match statement {
            StatementNode::LocalData(local) if local.symbol == symbol => Some(local.initial_value),
            StatementNode::Assignment(assignment) => {
                let target = validation::canonical_place_from_expression_in_state(
                    program,
                    state.symbol,
                    statement_index,
                    assignment.target,
                )?;
                (target.root == PlaceRoot::Symbol(symbol) && target.segments.is_empty())
                    .then_some(assignment.value)
            }
            _ => None,
        })
}

pub(crate) fn local_reference_storage_before_statement(
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    state: &FlowStateFact,
    index: usize,
    mut place: CanonicalPlace,
) -> Option<CanonicalPlace> {
    let root = place.root.as_symbol()?;
    let state = program.state_by_symbol(state.state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let order = program
        .statement_table
        .admitted_execution_order(state.statement_nodes);
    let end = order
        .position(index)
        .or_else(|| (index == statements.len()).then_some(order.indices().len()))?;
    let mut declarations = order.indices()[..end]
        .iter()
        .filter_map(|index| statements.get(*index))
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
    let reference = program
        .type_reference_table
        .unconstrained_type_reference(local.type_reference);
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
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
    observation: super::ValueObservation,
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
        observation,
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
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    borrow: &checked_trees::BorrowFacts,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    place: CanonicalPlace,
) -> Option<Vec<CanonicalPlace>> {
    let root = place.root.as_symbol()?;
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
    program: &dyn ::validation::CheckingInput,
    frames: &validation::CallFrameResolver<'_>,
    machine: &Machine,
    flow: &FlowFacts,
    state: &FlowStateFact,
    call: &FlowCallFact,
    places: &[CanonicalPlace],
    observation: super::ValueObservation,
) -> Option<()> {
    let position = call_position_in_state(flow, state, call)?;
    let calls = flow.control.calls.span_or_empty(state.calls);
    for prior in calls[..position]
        .iter()
        .filter(|prior| prior.statement_index == call.statement_index)
    {
        let site = validation::find_call_site(
            program,
            machine.symbol,
            state.state_symbol,
            prior.statement_index,
            prior.call_ordinal,
        )?;
        match site {
            validation::CallSite::Statement(call) => {
                if !frames.call_reference_bindings_are_stable(machine, call) {
                    return None;
                }
            }
            validation::CallSite::Expression { expression, .. } => {
                if !frames.expression_reference_bindings_are_stable(machine, expression) {
                    return None;
                }
            }
            validation::CallSite::TransitionNamed { .. } => return None,
        };
        for place in places {
            let preserved = match observation {
                super::ValueObservation::Contents => {
                    super::operand_contexts::entry_origin_is_stable
                }
                super::ValueObservation::Binding => {
                    super::operand_contexts::binding_origin_is_stable
                }
            };
            preserved(
                program,
                &flow.control,
                prior.normal_return_writes,
                !prior.normal_return_writes_complete,
                place,
            )
            .then_some(())?;
        }
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
    program: &dyn ::validation::CheckingInput,
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
            None,
            borrow_call,
            Some(frames),
        )?;
        if places.iter().any(|place| {
            writes.iter().any(|write| {
                place.root == write.root
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
