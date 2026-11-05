//! Reference-result geometry adapts the shared chronological result relation.
//!
//! Completed Flow operands can select stored descriptor leaves from an owned
//! input. The older raw storage query has only a borrowed input address and
//! declines that descriptor load. Both consumers use the same complete body
//! relation; neither replays a declaration initializer or creates loan authority.
//! Callee-local selectors never become caller coordinates. Closed-array
//! geometry retains every possible slot when an immutable actual cannot narrow
//! the selection; recast presentations still require their own correspondence.
//!
//! An unchanged place-rebase is not a referent. A reference binding contributes
//! storage only when its initializer or a distinct rebase names that storage.
//! A dynamic index denotes every fixed slot, and every slot must resolve; one
//! unresolved alternative fails the query.

use crate::flow::expressions;
use crate::published::places::place_at;
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
            if place_at(program, state_symbol, statement_index, assignment.target)
            .is_some_and(|place| place.root == PlaceRoot::Symbol(local_symbol) && place.segments.is_empty()))
    }) {
        return None;
    }
    let call = program.expression_table.call_node(local.initial_value)?;
    call_result_candidates(
        program,
        state_symbol,
        binding_index,
        local.initial_value,
        call,
        call_frames,
        false,
    )
}

fn call_result_candidates(
    program: &dyn ::validation::CheckingInput,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_site: ExpressionHandle,
    call: &TableCallExpression,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    include_shared: bool,
) -> Option<Vec<CanonicalPlace>> {
    let mut candidates = Vec::new();
    let arguments = program.expression_table.expression_handles(call.arguments);
    for source in call_result_leaf_sources(program, call, call_frames, &[])? {
        // This query has no completed descriptor inventory. An owned formal's
        // field denotes a stored descriptor, not its pointee address; only the
        // evaluated Flow consumer can load that leaf from its captured value.
        if !source.result_path.is_empty() || source.collection_coarse {
            return None;
        }
        match source.backing {
            ReferenceResultBacking::CallerArgument {
                argument_index,
                parameter_type,
            } => {
                if !parameter_grants_readable_loan(program, parameter_type, include_shared) {
                    return None;
                }
                let actual = *arguments.get(argument_index)?;
                for mut storage in storage_places(
                    program,
                    caller_state_symbol,
                    statement_index,
                    actual,
                    call_frames,
                    include_shared,
                )? {
                    storage.segments.extend_from_slice(&source.segments);
                    crate::shared::collections::push_unique(&mut candidates, storage);
                }
            }
            ReferenceResultBacking::InvocationResult(_) => {
                // The referent's opaque backing names its producing call site
                // — the same identity the performed write records — with the
                // leaf's referent-relative projection beneath it.
                crate::shared::collections::push_unique(
                    &mut candidates,
                    CanonicalPlace {
                        root: PlaceRoot::Expression(call_site),
                        segments: source.segments.clone(),
                    },
                );
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
                expression,
                call,
                frames,
                include_shared,
            );
        }
        ExpressionNode::Match(dispatch) => {
            let mut candidates = Vec::new();
            for arm in program.expression_table.match_arms(dispatch.arms) {
                for storage in storage_places(
                    program,
                    state,
                    statement_index,
                    arm.value,
                    frames,
                    include_shared,
                )? {
                    crate::shared::collections::push_unique(&mut candidates, storage);
                }
            }
            return (!candidates.is_empty()).then_some(candidates);
        }
        _ => {}
    }
    let place = place_at(program, state, statement_index, expression)?;
    storage_places_for_canonical_place(
        program,
        state,
        statement_index,
        &place,
        frames,
        include_shared,
        16,
    )
}

/// Storage a transition or call argument denotes. Reference bindings resolve
/// to their referents; an unresolved reference is not itself storage.
pub(crate) fn argument_storage_places(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    storage_places_for_canonical_place(program, state, statement_index, place, frames, true, 16)
}

fn storage_places_for_canonical_place(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    frames: Option<&validation::CallFrameResolver<'_>>,
    include_shared: bool,
    depth: usize,
) -> Option<Vec<CanonicalPlace>> {
    if depth == 0 {
        return None;
    }
    if place
        .segments
        .iter()
        .any(|segment| matches!(segment, PlaceSegment::Index { .. }))
    {
        let root = CanonicalPlace {
            root: place.root,
            segments: Vec::new(),
        };
        let root_type =
            validation::canonical_place_type_reference(program, state, statement_index, &root)?;
        let paths = validation::completed_storage::finite_projection_paths(
            program,
            root_type,
            &place.segments,
        )?;
        let mut candidates = Vec::new();
        for segments in paths {
            let fixed = CanonicalPlace {
                root: place.root,
                segments,
            };
            for storage in storage_places_for_canonical_place(
                program,
                state,
                statement_index,
                &fixed,
                frames,
                include_shared,
                depth - 1,
            )? {
                crate::shared::collections::push_unique(&mut candidates, storage);
            }
        }
        return (!candidates.is_empty()).then_some(candidates);
    }

    if let PlaceRoot::Expression(expression) = place.root {
        match program.expression_table.expression(expression) {
            ExpressionNode::Member(member) => {
                let symbol = validation::effective_member_symbol(program, member.receiver, member);
                let mut segments = Vec::new();
                validation::push_field_place_segments(program, &mut segments, symbol);
                segments.extend_from_slice(&place.segments);
                if let Some(mut receiver) =
                    place_at(program, state, statement_index, member.receiver)
                {
                    receiver.segments.extend(segments);
                    if receiver.root != place.root || receiver.segments != place.segments {
                        return storage_places_for_canonical_place(
                            program,
                            state,
                            statement_index,
                            &receiver,
                            frames,
                            include_shared,
                            depth - 1,
                        );
                    }
                }
            }
            ExpressionNode::Indexed(indexed) => {
                let mut segments = vec![validation::index_place_segment(program, indexed.index)];
                segments.extend_from_slice(&place.segments);
                if let Some(mut collection) =
                    place_at(program, state, statement_index, indexed.collection)
                {
                    collection.segments.extend(segments);
                    if collection.root != place.root || collection.segments != place.segments {
                        return storage_places_for_canonical_place(
                            program,
                            state,
                            statement_index,
                            &collection,
                            frames,
                            include_shared,
                            depth - 1,
                        );
                    }
                }
            }
            ExpressionNode::Borrow(borrow) => {
                if let Some(mut target) = place_at(program, state, statement_index, borrow.target) {
                    target.segments.extend_from_slice(&place.segments);
                    if target.root != place.root || target.segments != place.segments {
                        return storage_places_for_canonical_place(
                            program,
                            state,
                            statement_index,
                            &target,
                            frames,
                            include_shared,
                            depth - 1,
                        );
                    }
                }
            }
            ExpressionNode::Match(dispatch) => {
                let mut candidates = Vec::new();
                for arm in program.expression_table.match_arms(dispatch.arms) {
                    let mut arm_place = place_at(program, state, statement_index, arm.value)
                        .unwrap_or(CanonicalPlace {
                            root: PlaceRoot::Expression(arm.value),
                            segments: Vec::new(),
                        });
                    arm_place.segments.extend_from_slice(&place.segments);
                    for storage in storage_places_for_canonical_place(
                        program,
                        state,
                        statement_index,
                        &arm_place,
                        frames,
                        include_shared,
                        depth - 1,
                    )? {
                        crate::shared::collections::push_unique(&mut candidates, storage);
                    }
                }
                return (!candidates.is_empty()).then_some(candidates);
            }
            _ => {}
        }
    }

    let state_decl = program.state_by_symbol(state);
    let root_symbol = place.root.as_symbol();
    if let Some((binding_index, local)) = root_symbol
        .zip(state_decl)
        .and_then(|(root, decl)| program.state_local_declaration(decl, root))
        .filter(|&(ordinal, _)| ordinal < statement_index)
    {
        if parameter_grants_readable_loan(program, local.type_reference, include_shared) {
            let statements = program
                .statement_table
                .statements(state_decl?.statement_nodes);
            let prefix = statements.get(..statement_index)?;
            let rebound = prefix[binding_index + 1..].iter().any(|statement| {
                matches!(statement, StatementNode::Assignment(assignment)
                if place_at(program, state, statement_index, assignment.target)
                    .is_some_and(|target| {
                        target.root == place.root && target.segments.is_empty()
                    }))
            });
            if !rebound
                && let Some(initial) = storage_places(
                    program,
                    state,
                    binding_index,
                    local.initial_value,
                    frames,
                    include_shared,
                )
            {
                let mut candidates = Vec::new();
                for mut candidate in initial {
                    candidate.segments.extend_from_slice(&place.segments);
                    if candidate.root == place.root && candidate.segments == place.segments {
                        return None;
                    }
                    for storage in storage_places_for_canonical_place(
                        program,
                        state,
                        binding_index,
                        &candidate,
                        frames,
                        include_shared,
                        depth - 1,
                    )? {
                        crate::shared::collections::push_unique(&mut candidates, storage);
                    }
                }
                if !candidates.is_empty() {
                    return Some(candidates);
                }
            }
            if let Some(storage) = crate::flow::rebase_exact_local_place(
                program,
                state,
                statement_index,
                place.clone(),
                frames,
            ) && storage != *place
                && matches!(storage.root, PlaceRoot::Symbol(_))
            {
                return storage_places_for_canonical_place(
                    program,
                    state,
                    statement_index,
                    &storage,
                    frames,
                    include_shared,
                    depth - 1,
                );
            }
            return None;
        }
        if !place.segments.is_empty() {
            if let Some(projections) = crate::flow::literal_expression_projections(
                program,
                local.initial_value,
                local.type_reference,
                &place.segments,
                false,
            ) {
                let mut candidates = Vec::new();
                for projection in projections {
                    for mut storage in storage_places(
                        program,
                        state,
                        binding_index,
                        projection.expression,
                        frames,
                        include_shared,
                    )? {
                        storage.segments.extend_from_slice(&projection.remaining);
                        if storage.segments.is_empty()
                            && !place_is_readable_reference(
                                program,
                                state,
                                binding_index,
                                &storage,
                                include_shared,
                            )
                        {
                            crate::shared::collections::push_unique(&mut candidates, storage);
                            continue;
                        }
                        for resolved in storage_places_for_canonical_place(
                            program,
                            state,
                            binding_index,
                            &storage,
                            frames,
                            include_shared,
                            depth - 1,
                        )? {
                            crate::shared::collections::push_unique(&mut candidates, resolved);
                        }
                    }
                }
                if !candidates.is_empty() {
                    return Some(candidates);
                }
            }
            if let Some(mut source) = place_at(program, state, binding_index, local.initial_value) {
                source.segments.extend_from_slice(&place.segments);
                if source.root != place.root || source.segments != place.segments {
                    return storage_places_for_canonical_place(
                        program,
                        state,
                        binding_index,
                        &source,
                        frames,
                        include_shared,
                        depth - 1,
                    );
                }
            }
        }
    }

    if let Some(parameter) = root_symbol
        .zip(state_decl)
        .and_then(|(root, decl)| program.state_parameter_by_symbol(decl, root))
        && parameter_grants_readable_loan(program, parameter.type_reference, include_shared)
    {
        if let Some(storage) = crate::flow::rebase_exact_local_place(
            program,
            state,
            statement_index,
            place.clone(),
            frames,
        ) && storage != *place
            && matches!(storage.root, PlaceRoot::Symbol(_))
        {
            return storage_places_for_canonical_place(
                program,
                state,
                statement_index,
                &storage,
                frames,
                include_shared,
                depth - 1,
            );
        }
        // A reference parameter is the subject of its entry qualification.
        // The binding is not a local slot that still needs a referent.
        return Some(vec![place.clone()]);
    }

    // An expression root that did not unpeel to a symbol is not storage.
    // A reference-typed place that did not reach its referent is not storage.
    if place.root.as_symbol().is_none()
        || place_is_readable_reference(program, state, statement_index, place, include_shared)
    {
        return None;
    }
    Some(vec![place.clone()])
}

fn place_is_readable_reference(
    program: &dyn ::validation::CheckingInput,
    state: SymbolHandle,
    statement_index: usize,
    place: &CanonicalPlace,
    include_shared: bool,
) -> bool {
    validation::canonical_place_type_reference(program, state, statement_index, place)
        .is_some_and(|reference| parameter_grants_readable_loan(program, reference, include_shared))
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

/// What a returned leaf's completed origin names. A caller route projects
/// an actual's storage through its formal's declared type; an opaque result
/// referent names backing the completed invocation owns, typed by the
/// declared referee. Neither kind supplies new loan authority.
#[derive(Clone, Copy)]
pub(in crate::flow) enum ReferenceResultBacking {
    CallerArgument {
        argument_index: usize,
        parameter_type: typed_trees::types::TypeReferenceHandle,
    },
    InvocationResult(validation::InvocationResultHome),
}

pub(in crate::flow) struct ReferenceResultSource {
    pub(in crate::flow) kind: ReferenceResultSourceKind,
    pub(in crate::flow) result_path: Vec<checked_trees::CheckedUnitStructuralPathSegment>,
    pub(in crate::flow) backing: ReferenceResultBacking,
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
    indices: &[Option<usize>],
) -> Option<Vec<ReferenceResultSource>> {
    let sources = call_result_captured_sources(program, call, call_frames, indices)?;
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
    // Requirement members carry no stored machine state: their signature is
    // the callee contract. A requirement call's receiver is the authored
    // trait path, which binds no storage lane, so the attached-receiver
    // topology check applies to stored callees only.
    let (parameters, stored_attached_receiver) = if let Some((callee, state)) =
        validation::find_state(program, entered_target)
    {
        (
            program.state_parameters(state),
            Some(callee.attached_data.is_some()),
        )
    } else {
        let (_, signature) = validation::selected_requirement_signature(program, entered_target)?;
        (program.state_signature_parameters(signature), None)
    };
    if stored_attached_receiver.is_some_and(|attached| call.receiver.is_valid() != attached)
        || !call.carries_only_positional_arguments()
        || !call.selects_only_nominal_route()
        || expressions(program)
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
        frames.reference_result_transports(entered_target)?
    } else {
        frames.reference_result_transports_for_indices(entered_target, indices)?
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
                        | PlaceSegment::Index { .. }
                )
            })
        {
            return None;
        }
        let result_path = crate::borrow::structural_owner_path(
            program,
            &crate::borrow::owner_path_from_place_segments(program, &result_path),
        )?;
        if let Some(home) = source.invocation_result {
            // A returned opaque referent's finite projection replays against
            // its declared referee; no formal lane exists to enumerate.
            for segments in validation::completed_storage::finite_projection_paths(
                program,
                home.referent,
                &source.segments,
            )? {
                sources.push(ReferenceResultSource {
                    kind,
                    result_path: result_path.clone(),
                    backing: ReferenceResultBacking::InvocationResult(home),
                    collection_coarse: source.collection_coarse,
                    segments,
                });
            }
            continue;
        }
        let (argument_index, parameter) = parameters
            .iter()
            .enumerate()
            .find(|(_, parameter)| parameter.symbol == source.root)?;
        // This is a complete alternative roster, not a claim that all slots
        // were selected. The descriptor observation owns the selected value;
        // every possible backing remains a loan/invalidation obligation.
        for segments in validation::completed_storage::finite_projection_paths(
            program,
            parameter.type_reference,
            &source.segments,
        )? {
            sources.push(ReferenceResultSource {
                kind,
                result_path: result_path.clone(),
                backing: ReferenceResultBacking::CallerArgument {
                    argument_index,
                    parameter_type: parameter.type_reference,
                },
                collection_coarse: source.collection_coarse,
                segments,
            });
        }
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
