//! Reference-result geometry adapts the shared chronological result relation.
//!
//! Completed Flow operands can select stored descriptor leaves from an owned
//! input. The older raw storage query has only a borrowed input address and
//! declines that descriptor load. Both consumers use the same complete body
//! relation; neither replays a declaration initializer or creates loan authority.
//! Callee-local selectors never become caller coordinates. Closed-array
//! geometry retains every possible slot when an immutable actual cannot narrow
//! the selection; recast presentations still require their own correspondence.

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
    reference_result_candidates_before_statement_including_shared(
        program,
        state_symbol,
        statement_index,
        local_symbol,
        call_frames,
        false,
    )
}

pub(crate) fn reference_result_candidates_before_statement_including_shared(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    statement_index: usize,
    local_symbol: SymbolHandle,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    include_shared: bool,
) -> Option<Vec<CanonicalPlace>> {
    let state = program.state_by_symbol(state_symbol)?;
    let statements = program.statement_table.statements(state.statement_nodes);
    let prefix = statements.get(..statement_index)?;
    let (binding_index, local) = program
        .state_local_declaration(state, local_symbol)
        .filter(|&(ordinal, _)| ordinal < statement_index)?;
    if !parameter_grants_readable_loan(program, local.type_reference, include_shared) {
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
    storage_places(
        program,
        state_symbol,
        binding_index,
        local.initial_value,
        call_frames,
        include_shared,
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
        let ty_ref = validation::canonical_place_type_reference(
            program,
            state,
            statement_index,
            &root,
        )?;
        let paths = validation::completed_storage::finite_projection_paths(
            program,
            ty_ref,
            &place.segments,
        )?;
        let mut candidates = Vec::new();
        for segments in paths {
            let fixed = CanonicalPlace {
                root: place.root,
                segments,
            };
            if let Some(storages) = storage_places_for_canonical_place(
                program,
                state,
                statement_index,
                &fixed,
                frames,
                include_shared,
                depth - 1,
            ) {
                for storage in storages {
                    crate::shared::collections::push_unique(&mut candidates, storage);
                }
            }
        }
        return (!candidates.is_empty()).then_some(candidates);
    }

    if let PlaceRoot::Expression(expr) = place.root {
        match program.expression_table.expression(expr) {
            ExpressionNode::Member(member) => {
                let member_symbol = validation::effective_member_symbol(program, member.receiver, member);
                let receiver_place = place_at(program, state, statement_index, member.receiver);
                let mut segments = Vec::new();
                validation::push_field_place_segments(program, &mut segments, member_symbol);
                segments.extend_from_slice(&place.segments);
                if let Some(mut receiver_place) = receiver_place {
                    receiver_place.segments.extend_from_slice(&segments);
                    return storage_places_for_canonical_place(
                        program,
                        state,
                        statement_index,
                        &receiver_place,
                        frames,
                        include_shared,
                        depth - 1,
                    );
                }
            }
            ExpressionNode::Indexed(indexed) => {
                let segment = validation::index_place_segment(program, indexed.index);
                let mut segments = vec![segment];
                segments.extend_from_slice(&place.segments);
                if let Some(mut coll_place) = place_at(program, state, statement_index, indexed.collection) {
                    coll_place.segments.extend_from_slice(&segments);
                    return storage_places_for_canonical_place(
                        program,
                        state,
                        statement_index,
                        &coll_place,
                        frames,
                        include_shared,
                        depth - 1,
                    );
                }
            }
            ExpressionNode::Borrow(borrow) => {
                if let Some(mut target_place) = place_at(program, state, statement_index, borrow.target) {
                    target_place.segments.extend_from_slice(&place.segments);
                    return storage_places_for_canonical_place(
                        program,
                        state,
                        statement_index,
                        &target_place,
                        frames,
                        include_shared,
                        depth - 1,
                    );
                }
            }
            _ => {}
        }
    }

    let root_symbol = place.root.as_symbol();
    let state_decl = program.state_by_symbol(state);
    let local_decl = root_symbol
        .and_then(|root| {
            state_decl.and_then(|decl| program.state_local_declaration(decl, root))
        })
        .filter(|&(ordinal, _)| ordinal < statement_index);

    if let Some((binding_index, local)) = local_decl {
        if parameter_grants_readable_loan(program, local.type_reference, include_shared) {
            let statements = program
                .statement_table
                .statements(state_decl.unwrap().statement_nodes);
            let prefix = statements.get(..statement_index)?;
            let rebound = prefix[binding_index + 1..].iter().any(|statement| {
                matches!(statement, StatementNode::Assignment(assignment)
                    if place_at(program, state, statement_index, assignment.target)
                    .is_some_and(|target| target.root == place.root && target.segments.is_empty()))
            });
            if !rebound {
                let mut candidates = Vec::new();
                if let Some(initial_storages) = storage_places(
                    program,
                    state,
                    binding_index,
                    local.initial_value,
                    frames,
                    include_shared,
                ) {
                    for mut cand in initial_storages {
                        cand.segments.extend_from_slice(&place.segments);
                        if cand.segments.is_empty() || cand == *place {
                            crate::shared::collections::push_unique(&mut candidates, cand);
                        } else if let Some(resolved) = storage_places_for_canonical_place(
                            program,
                            state,
                            binding_index,
                            &cand,
                            frames,
                            include_shared,
                            depth - 1,
                        ) {
                            for r in resolved {
                                crate::shared::collections::push_unique(&mut candidates, r);
                            }
                        } else {
                            crate::shared::collections::push_unique(&mut candidates, cand);
                        }
                    }
                    if !candidates.is_empty() {
                        return Some(candidates);
                    }
                }
            }
        } else if !place.segments.is_empty() {
            if let Some(projections) = crate::flow::literal_expression_projections(
                program,
                local.initial_value,
                local.type_reference,
                &place.segments,
                false,
            ) {
                let mut candidates = Vec::new();
                for proj in projections {
                    if let Some(mut storages) = storage_places(
                        program,
                        state,
                        binding_index,
                        proj.expression,
                        frames,
                        include_shared,
                    ) {
                        for storage in &mut storages {
                            storage.segments.extend_from_slice(&proj.remaining);
                            crate::shared::collections::push_unique(
                                &mut candidates,
                                storage.clone(),
                            );
                        }
                    }
                }
                if !candidates.is_empty() {
                    return Some(candidates);
                }
            }
            if let Some(mut source) = place_at(program, state, binding_index, local.initial_value) {
                source.segments.extend_from_slice(&place.segments);
                if let Some(candidates) = storage_places_for_canonical_place(
                    program,
                    state,
                    binding_index,
                    &source,
                    frames,
                    include_shared,
                    depth - 1,
                ) {
                    return Some(candidates);
                }
            }
        }
    }

    let root = place.root.as_symbol()?;
    let state_decl = program.state_by_symbol(state);
    let root_type = if let Some((_, local)) = state_decl
        .and_then(|decl| program.state_local_declaration(decl, root))
        .filter(|&(ordinal, _)| ordinal < statement_index)
    {
        Some(local.type_reference)
    } else if let Some(param) = state_decl.and_then(|decl| program.state_parameter_by_symbol(decl, root)) {
        Some(param.type_reference)
    } else {
        None
    };

    if let Some(ty) = root_type {
        if parameter_grants_readable_loan(program, ty, include_shared) {
            if let Some(storage) = crate::flow::rebase_exact_local_place(
                program,
                state,
                statement_index,
                place.clone(),
                frames,
            ) {
                if storage != *place && matches!(storage.root, PlaceRoot::Symbol(_)) {
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
            }
            return None;
        }
    }

    Some(vec![place.clone()])
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
