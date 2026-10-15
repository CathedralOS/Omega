//! Checked reference-result sources for loan authority and storage facts.
//!
//! `let room: &mut Room = level.room_mut(cell)` binds a reference whose
//! referent is one of the places the callee's exits return -- here the
//! sixteen `&mut slots[i]` arms over `self.rooms` -- and the returned view
//! carries the callee's loans into the caller's actual, not a borrow of any
//! private storage (wiki/spec/language/lifetimes.md, "Returned views"). The
//! frame resolver knows no single origin for such a local, so an exclusive
//! actual spelled through it left every call frame unrepresentable. This
//! trace enumerates the callee's returned places through its `&mut`
//! parameters (`self` included) and maps each onto the caller's actual, in
//! the existing place vocabulary: `level.rooms[0]` .. `level.rooms[15]`.
//!
//! Fail-closed gates mirror the owned-result trace in
//! `checks/termination/progress/origins.rs`: the callee must be a nongeneric
//! checked body called without machine, evidence or dispatch arguments;
//! every exit of its entry state must return a place rooted at an exclusive
//! parameter through field, case and literal-index segments only; a
//! sub-state route, a runtime index, or an unresolved callee local yields no
//! candidates and the caller keeps its conservative treatment.
//! Lifetime binders are erased regions, not runtime parameters. They do not
//! change this source substitution; ordinary lifetime/escape checks still
//! establish whether the returned access and source relation are legal.

use validation::CanonicalPlace;

use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::statement::{StatementNode, TransitionExit, TransitionTargetNode};

/// The candidate storage places a reference-typed local names at
/// `statement_index`, when it was bound from a checked reference result and
/// has not been rebound since. `None` keeps the conservative treatment.
pub(crate) fn reference_result_candidates_before_statement(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    local_symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let state = program.state_by_symbol(state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..statement_index)?;
    let (binding_index, local) = program
        .state_local_declaration(state, local_symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)?;
    if !crate::checks::contracts::is_readable_mutable_reference(program, local.type_reference) {
        return None;
    }
    // A later bare assignment may rebind the reference to another result.
    if prefix[binding_index + 1..].iter().any(|statement| {
        matches!(statement, StatementNode::Assignment(assignment)
            if validation::canonical_place_from_expression_in_state(
                program,
                state_symbol,
                statement_index,
                assignment.target,
            )
            .is_some_and(|place| place.root == PlaceRoot::Symbol(local_symbol) && place.segments.is_empty()))
    }) {
        return None;
    }
    let Some(call) = program.expression_table.call_node(local.initial_value) else {
        return None;
    };
    call_result_candidates(program, state_symbol, binding_index, call, call_frames)
}

fn call_result_candidates(
    program: &dyn ::validation::CheckingInput,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call: &TableCallExpression,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let mut candidates = Vec::new();
    for source in call_result_sources(program, call, call_frames)? {
        for mut storage in reference_expression_storage_places(
            program,
            caller_state_symbol,
            statement_index,
            source.actual,
            call_frames,
        )? {
            storage.segments.extend_from_slice(&source.segments);
            if !candidates.contains(&storage) {
                candidates.push(storage);
            }
        }
    }
    (!candidates.is_empty()).then_some(candidates)
}

/// Backing storage of a reference expression for effect/range invalidation.
/// This is not loan authority: a borrow consumer must also establish the live
/// source loan and parent relationship before granting exclusive access.
pub(crate) fn reference_expression_storage_places(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            return reference_expression_storage_places(
                program,
                state,
                statement_index,
                borrow.target,
                frames,
            );
        }
        ExpressionNode::Call(call) => {
            return call_result_candidates(program, state, statement_index, call, frames);
        }
        _ => {}
    }
    let place = validation::canonical_place_from_expression_in_state(
        program,
        state,
        statement_index,
        expression,
    )?;
    if let Some(storage) = crate::flow::rebase_exact_local_place(
        program,
        state,
        statement_index,
        place.clone(),
        frames,
    ) {
        return matches!(storage.root, PlaceRoot::Symbol(_)).then_some(vec![storage]);
    }
    let root = place.root.as_symbol()?;
    let mut candidates = reference_result_candidates_before_statement(
        program,
        state,
        statement_index,
        root,
        frames,
    )?;
    for candidate in &mut candidates {
        candidate.segments.extend_from_slice(&place.segments);
    }
    Some(candidates)
}

/// A checked body's actual storage route, before rebasing through caller
/// locals. This value-origin query is not the lifetime/authority frontier:
/// declarations can retain inputs absent from the body's actual return paths.
struct ReferenceResultSource {
    actual: ExpressionHandle,
    segments: Vec<PlaceSegment>,
}

fn call_result_sources(
    program: &dyn ::validation::CheckingInput,
    call: &TableCallExpression,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ReferenceResultSource>> {
    let (callee, callee_state) = validation::find_state(program, call.target_symbol)?;
    if callee.supply_mode != language_semantics::MachineSupplyMode::CheckedBody
        || !callee.body_is_present
        || !program.machine_type_parameters(callee).is_empty()
        || call.receiver.is_valid() != callee.attached_data.is_some()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
        || program.machine_states(callee).first()?.symbol != callee_state.symbol
    {
        return None;
    }
    let parameters = program.state_parameters(callee_state);
    let arguments = program.expression_table.expression_handles(call.arguments);
    if arguments.len() != parameters.iter().count() {
        return None;
    }
    let mut sources = Vec::new();
    for (returned_index, returned) in callee_returned_expressions(program, callee_state)? {
        let place = validation::canonical_place_from_expression_in_state(
            program,
            callee_state.symbol,
            returned_index,
            returned,
        )?;
        // A callee local (`let slots: &mut [Room] = &mut self.rooms[..]`)
        // resolves to its exact origin; anything less is no candidate.
        let place = crate::flow::rebase_exact_local_place(
            program,
            callee_state.symbol,
            returned_index,
            place,
            call_frames,
        )?;
        let root = place.root.as_symbol()?;
        if !place.segments.iter().all(|segment| {
            matches!(
                segment,
                PlaceSegment::Field { .. }
                    | PlaceSegment::Case { .. }
                    | PlaceSegment::FixedIndex { .. }
            )
        }) {
            return None;
        }
        let parameter = parameters
            .iter()
            .find(|parameter| parameter.symbol == root)?;
        if !crate::checks::contracts::is_readable_mutable_reference(
            program,
            parameter.type_reference,
        ) {
            return None;
        }
        let actual = {
            *arguments.get(
                parameters
                    .iter()
                    .position(|candidate| candidate.symbol == root)?,
            )?
        };
        sources.push(ReferenceResultSource {
            actual,
            segments: place.segments,
        });
    }
    (!sources.is_empty()).then_some(sources)
}

/// Every returned expression of the callee's entry state with its statement
/// index: each value arm of its transition run and a trailing result
/// expression. A route into another state, a self-loop, or a terminal exit
/// cannot be enumerated here.
fn callee_returned_expressions(
    program: &dyn ::validation::CheckingInput,
    state: &typed_trees::state::State,
) -> Option<Vec<(usize, ExpressionHandle)>> {
    let statements = program.statement_table.statements(state.statement_nodes);
    let mut returned = Vec::new();
    let mut has_transition = false;
    for (index, statement) in statements.iter().enumerate() {
        let Some(transition) = statement.as_transition() else {
            continue;
        };
        has_transition = true;
        if transition.exit != TransitionExit::Ordinary {
            return None;
        }
        for target in transition.targets() {
            match program.statement_table.transition_target(target) {
                TransitionTargetNode::Value(value) => returned.push((index, *value)),
                _ => return None,
            }
        }
    }
    if !has_transition {
        let (index, last) = statements.iter().enumerate().next_back()?;
        let value = last.as_expression()?;
        returned.push((index, *value));
    }
    Some(returned)
}
