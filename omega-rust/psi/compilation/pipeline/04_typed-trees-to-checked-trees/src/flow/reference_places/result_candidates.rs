//! Reference-result geometry adapts the shared chronological result relation.
//!
//! Completed Flow operands can select stored descriptor leaves from an owned
//! input. The older raw storage query has only a borrowed input address and
//! declines that descriptor load. Both consumers use the same complete body
//! relation; neither replays a declaration initializer or creates loan authority.
//! Callee-local dynamic selectors and recast presentations are not executable
//! caller coordinates and remain unavailable to this exact-address adapter.

use validation::CanonicalPlace;

use facts::{PlaceRoot, PlaceSegment};
use symbols::SymbolHandle;
use typed_trees::expression::{ExpressionHandle, ExpressionNode, TableCallExpression};
use typed_trees::statement::StatementNode;

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
    call_result_candidates(
        program,
        state_symbol,
        binding_index,
        call,
        call_frames,
        false,
    )
}

fn call_result_candidates(
    program: &dyn ::validation::CheckingInput,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call: &TableCallExpression,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    include_shared: bool,
) -> Option<Vec<CanonicalPlace>> {
    let mut candidates = Vec::new();
    let arguments = program.expression_table.expression_handles(call.arguments);
    for source in call_result_leaf_sources(program, call, call_frames)? {
        // This query has no completed descriptor inventory. An owned formal's
        // field denotes a stored descriptor, not its pointee address; only the
        // evaluated Flow consumer can load that leaf from its captured value.
        if !source.result_path.is_empty()
            || source.collection_coarse
            || !parameter_grants_readable_loan(program, source.parameter_type, include_shared)
        {
            return None;
        }
        let actual = *arguments.get(source.argument_index)?;
        for mut storage in storage_places(
            program,
            caller_state_symbol,
            statement_index,
            actual,
            call_frames,
            include_shared,
        )? {
            storage.segments.extend_from_slice(&source.segments);
            if !candidates.contains(&storage) {
                candidates.push(storage);
            }
        }
    }
    (!candidates.is_empty()).then_some(candidates)
}

/// Backing storage of mutable or shared reference expressions. Proof reads
/// the referent; this mapping alone does not establish loan authority.
pub(crate) fn reference_expression_storage_places_including_shared(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    storage_places(program, state, statement_index, expression, frames, true)
}

fn storage_places(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    expression: ExpressionHandle,
    frames: Option<&validation::CallFrameResolver<'_>>,
    include_shared: bool,
) -> Option<Vec<CanonicalPlace>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(borrow) => {
            return storage_places(
                program,
                state,
                statement_index,
                borrow.target,
                frames,
                include_shared,
            );
        }
        ExpressionNode::Call(call) => {
            return call_result_candidates(
                program,
                state,
                statement_index,
                call,
                frames,
                include_shared,
            );
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
#[derive(Clone, Copy)]
pub(in crate::flow) enum ReferenceResultSourceKind {
    Reference,
    Value {
        type_reference: typed_trees::types::TypeReferenceHandle,
    },
}

pub(in crate::flow) struct ReferenceResultSource {
    pub(in crate::flow) kind: ReferenceResultSourceKind,
    pub(in crate::flow) result_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(in crate::flow) argument_index: usize,
    pub(in crate::flow) parameter_type: typed_trees::types::TypeReferenceHandle,
    /// Indexed frame spellings coarsen to the containing collection even when
    /// authenticated structural selectors retain an exact fixed slot. Raw
    /// storage adapters need this limit; completed values consume the selectors.
    collection_coarse: bool,
    pub(in crate::flow) segments: Vec<PlaceSegment>,
}

pub(in crate::flow) fn call_result_leaf_sources(
    program: &dyn ::validation::CheckingInput,
    call: &TableCallExpression,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<ReferenceResultSource>> {
    let sources = call_result_captured_sources(program, call, call_frames, &[])?;
    sources
        .into_iter()
        .map(|source| {
            // Raw storage consumers have no completed value inventory with
            // which to select a descriptor from a transported owned subtree.
            matches!(source.kind, ReferenceResultSourceKind::Reference).then_some(source)
        })
        .collect()
}

/// Preserve whole-value transport until the completed-input consumer can
/// select its actual descriptor leaves. A declaration path identifies a slot,
/// never the presence or address of the value carried in that slot.
pub(in crate::flow) fn call_result_captured_sources(
    program: &dyn ::validation::CheckingInput,
    call: &TableCallExpression,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    indices: &[Option<usize>],
) -> Option<Vec<ReferenceResultSource>> {
    let entered_target = validation::call_target_entry_symbol(
        program,
        validation::call_target_symbol(program, call),
    );
    let (callee, state) = validation::find_state(program, entered_target)?;
    let parameters = program.state_parameters(state);
    if call.receiver.is_valid() != callee.attached_data.is_some()
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
        || program
            .expression_table
            .expression_handles(call.arguments)
            .len()
            != parameters.len()
    {
        return None;
    }
    let prepared_frames;
    let frames = match call_frames {
        Some(frames) => frames,
        None => {
            prepared_frames = validation::CallFrameResolver::new(program)?;
            &prepared_frames
        }
    };
    // Raw storage consumers have no completed argument roster. Captured
    // consumers supply every formal lane, including unknown coordinates;
    // only the shared invocation owner may refine the symbolic source paths.
    let transports = if indices.is_empty() {
        frames.reference_result_transports(state.symbol)?
    } else {
        frames.reference_result_transports_for_indices(state.symbol, indices)?
    };
    let mut sources = Vec::with_capacity(transports.len());
    for transport in transports {
        let (result_path, source, kind) = match transport {
            validation::ReferenceResultTransport::Reference(origin) => (
                origin.result_path,
                origin.source,
                ReferenceResultSourceKind::Reference,
            ),
            validation::ReferenceResultTransport::Value {
                result_path,
                source,
                type_reference,
            } => (
                result_path,
                source,
                ReferenceResultSourceKind::Value { type_reference },
            ),
        };
        // Every returned leaf keeps its own result coordinate. Every feasible
        // origin must be exact; omitting an unresolved alternative would invent
        // a more precise descriptor. Lifetime signatures cannot supply it.
        if source.is_image_storage
            || !source.recasts.is_empty()
            || source.representation.is_some()
            || !source.segments.iter().all(|segment| {
                matches!(
                    segment,
                    PlaceSegment::Field { .. }
                        | PlaceSegment::Case { .. }
                        | PlaceSegment::FixedIndex { .. }
                )
            })
        {
            return None;
        }
        let (argument_index, parameter) = parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == source.root)?;
        sources.push(ReferenceResultSource {
            kind,
            result_path: crate::borrow::structural_owner_path(
                program,
                &crate::borrow::owner_path_from_place_segments(program, &result_path),
            )?,
            argument_index,
            parameter_type: parameter.type_reference,
            collection_coarse: source.collection_coarse,
            segments: source.segments,
        });
    }
    (!sources.is_empty()).then_some(sources)
}

fn parameter_grants_readable_loan(
    program: &dyn ::validation::CheckingInput,
    type_reference: typed_trees::types::TypeReferenceHandle,
    include_shared: bool,
) -> bool {
    if !include_shared {
        return crate::checks::contracts::is_readable_mutable_reference(program, type_reference);
    }
    let mut reference = type_reference;
    while reference.is_valid() {
        match program.type_reference_table.type_reference(reference) {
            typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
                reference = *base_type;
            }
            typed_trees::types::TypeReferenceNode::Reference { access, .. } => {
                return *access != language_semantics::ReferenceAccess::WriteOnly;
            }
            _ => return false,
        }
    }
    false
}
