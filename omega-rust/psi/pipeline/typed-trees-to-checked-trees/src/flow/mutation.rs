use crate::flow::CanonicalPlace;
use crate::flow::canonical_place_from_expression_in_state;
use crate::flow::canonical_place_from_symbol;
use crate::flow::canonical_place_segments_may_overlap;
use crate::flow::normalize_attached_place_root;
use crate::flow::normalized_event_place_root;
use crate::lookup::expression_root_symbol;
use crate::semantic_calls::CallSite;
use crate::semantic_calls::call_site_argument_expressions;
use crate::semantic_calls::find_call_site;
use crate::semantic_calls::find_state;
use checked_trees::expression::ExpressionNode;
use checked_trees::statement::StatementNode;
use checked_trees::{BorrowCallFact, BorrowFacts};
use symbols::SymbolHandle;
mod ceiling;
mod local_origins;
mod operand_coordinates;
mod receiver;
mod summary;

pub(crate) use ceiling::signature_ceiling_places;
pub(crate) use local_origins::close_storage_places_over_aliases_with_resolver;
pub(crate) use local_origins::origin_place;
pub(crate) use local_origins::place_from_origin_path;
pub(crate) use local_origins::rebase_exact_local_place;
pub(crate) use receiver::{
    call_receiver_is_mutable, call_receiver_mutated_place, canonical_receiver_place_for_call_site,
};
pub(crate) use summary::StateMutationSummaryCache;
use summary::instantiate_known_call_mutation_summary_places;

#[derive(Clone, Copy)]
pub(super) enum WritePlaceNamespace {
    /// Preserve the caller's reference binding through which access occurs.
    AccessRoute,
    /// Rebase reference bindings to the storage whose facts may be invalidated.
    Storage,
}

/// Caller storage footprint. `None` requires full invalidation; an empty
/// complete footprint preserves facts. Neither may be represented by an
/// unresolved reference-binding root.
/// Reuse the resolver prepared for this immutable program; local origins still
/// resolve at the exact caller prefix, not from an earlier query's facts.
pub(crate) fn call_mutated_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    if !operand_coordinates::call_operands_have_builtin_coordinates(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        &site,
    ) {
        return None;
    }
    // An unknown frame retires no more than the declared-signature ceiling
    // (mutation/ceiling.rs); only an unrepresentable ceiling stays unknown
    // and retires every live fact.
    call_write_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        WritePlaceNamespace::Storage,
        call_frames,
    )
    .or_else(|| {
        ceiling::signature_ceiling_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            call_frames,
        )
    })
}

/// The access route retains the local loan owner; storage rebasing must not
/// turn a write through that loan into an independent write to its referent.
pub(crate) fn call_write_accesses(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    call_write_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        WritePlaceNamespace::AccessRoute,
        None,
    )
    .expect("access-route projection always retains the ownership fallback")
}

fn call_write_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let summarized_places = instantiate_known_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        namespace,
        call_frames,
    );
    let use_mutable_argument_fallback = summarized_places.is_none();
    let known_target_summary = summarized_places.is_some();
    let mut places = Vec::new();

    if let Some(summarized_places) = summarized_places {
        for place in summarized_places {
            if !places.contains(&place) {
                places.push(place);
            }
        }
    }

    if use_mutable_argument_fallback {
        for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
            if access.kind.is_exclusive()
                && let Some(mut place) = canonical_place_from_symbol(access.root_symbol)
            {
                place.extend_segments(borrow.access_segments.span_or_empty(access.segments));
                if !places.contains(&place) {
                    places.push(place);
                }
            }
        }

        if let Some(call_site) = find_call_site(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        ) && let Some(target_state) = find_state(program, borrow_call.target_symbol)
        {
            let mut argument_index = 0usize;
            for parameter in program.state_parameters(target_state) {
                if parameter.is_self {
                    continue;
                }

                let argument = call_site_argument_expressions(program, &call_site)
                    .get(argument_index)
                    .copied();
                argument_index = argument_index.saturating_add(1);

                if !parameter.is_mutable {
                    continue;
                }

                if let Some(argument) = argument
                    && let Some(place) = canonical_place_from_expression_in_state(
                        program,
                        caller_state_symbol,
                        borrow_call.statement_index,
                        argument,
                    )
                    && !places.contains(&place)
                {
                    places.push(place);
                }
            }
        }
    }

    if !known_target_summary
        && borrow_call.has_receiver
        && call_receiver_is_mutable(program, borrow, borrow_call)
        && let Some(place) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        && !places.contains(&place)
    {
        places.push(place);
    }

    if !known_target_summary && matches!(namespace, WritePlaceNamespace::Storage) {
        let boundary_target = program.traits().iter().any(|definition| {
            definition.is_boundary
                && program
                    .trait_machine_signatures(definition)
                    .iter()
                    .any(|signature| signature.symbol == borrow_call.target_symbol)
        });
        if boundary_target {
            // The shared boundary frame owns implicit receiver and argument
            // reach. Reconstruct its storage paths instead of using access
            // roots, which can name a field without its caller receiver.
            return shared_call_storage_places(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                borrow,
                borrow_call,
                call_frames,
            );
        }
        if places.is_empty() {
            return call_is_storage_free_asm_intrinsic(program, borrow_call).then(Vec::new);
        }
        let mut storage = Vec::new();
        for place in places {
            let canonical_places = local_origins::rebase_local_write_places(
                program,
                caller_state_symbol,
                borrow_call.statement_index,
                place,
                call_frames,
            )?;
            for canonical in canonical_places {
                if !storage.contains(&canonical) {
                    storage.push(canonical);
                }
            }
        }
        Some(storage)
    } else {
        Some(places)
    }
}

fn shared_call_storage_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == caller_machine_symbol)?;
    let state = find_state(program, caller_state_symbol)?;
    let site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let resolver = call_frames?;
    let frame = match &site {
        CallSite::Statement(call) => resolver.may_write_frame(machine, call),
        CallSite::Expression { expression, .. } => {
            resolver.expression_write_frame(machine, *expression)
        }
        CallSite::TransitionNamed { .. } => return None,
    };
    let (exclusive_referents, coarse_anchors, refinable) = boundary_frame_access_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        &site,
        call_frames,
    );
    let mut places = Vec::new();
    for path in frame.complete_paths()? {
        let source = local_origins::place_from_origin_path(
            program,
            state,
            borrow_call.statement_index,
            path,
        )?;
        for coarse in local_origins::rebase_local_write_places(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            source,
            call_frames,
        )? {
            // The durable frame path deliberately drops index selectors to the
            // collection (`self.cells[1].out` renders as `self.cells`), while
            // the recorded `&mut`/`&write` argument accesses retain the exact
            // lent referent. Refine the coarse storage place to those exact
            // referents only while every other contribution that renders to
            // the same path — a non-exclusive access or a mutable receiver —
            // keeps it from narrowing; otherwise emit the collection.
            let identity = storage_normalized_place(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &coarse,
            );
            let blocked = !refinable
                || coarse_anchors
                    .iter()
                    .any(|anchor| storage_extends_frame_place(program, anchor, &identity));
            let refinements: Vec<CanonicalPlace> = if blocked {
                Vec::new()
            } else {
                exclusive_referents
                    .iter()
                    .filter(|referent| storage_extends_frame_place(program, referent, &identity))
                    .map(|referent| CanonicalPlace {
                        root: coarse.root,
                        segments: referent.segments.clone(),
                    })
                    .collect()
            };
            if refinements.is_empty() {
                if !places.contains(&coarse) {
                    places.push(coarse);
                }
                continue;
            }
            for refined in refinements {
                if !places.contains(&refined) {
                    places.push(refined);
                }
            }
        }
    }
    Some(places)
}

/// The call's recorded argument accesses in the storage namespace frame paths
/// resolve to: exclusive borrow referents that may refine a coarsened path, and
/// the other contributions that render to the same coarse path and must keep
/// it — a non-`borrow` actual on an exclusive parameter carries a reference
/// whose referent is only coarsely accounted, and the receiver contributes its
/// own place. `false` disables refinement entirely when an exclusive actual
/// cannot be spelled as caller storage at all.
fn boundary_frame_access_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    site: &CallSite<'_>,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> (Vec<CanonicalPlace>, Vec<CanonicalPlace>, bool) {
    let mut exclusive_referents = Vec::new();
    let mut coarse_anchors = Vec::new();
    let mut refinable = true;
    for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
        if !access.kind.is_exclusive() {
            continue;
        }
        let Some(mut place) = canonical_place_from_symbol(access.root_symbol) else {
            refinable = false;
            break;
        };
        place.extend_segments(borrow.access_segments.span_or_empty(access.segments));
        let Some(rebased) = local_origins::rebase_local_write_places(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            place,
            call_frames,
        ) else {
            refinable = false;
            break;
        };
        for rebased in rebased {
            let rebased = storage_normalized_place(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &rebased,
            );
            if !exclusive_referents.contains(&rebased) {
                exclusive_referents.push(rebased);
            }
        }
    }
    if refinable
        && let Some(parameters) =
            crate::semantic_calls::call_target_parameters(program, borrow_call.target_symbol)
    {
        let arguments = call_site_argument_expressions(program, site);
        if parameters
            .iter()
            .filter(|parameter| !parameter.is_self)
            .count()
            != arguments.len()
        {
            refinable = false;
        } else {
            let mut argument_index = 0usize;
            for parameter in parameters {
                if parameter.is_self {
                    continue;
                }
                let argument = arguments[argument_index];
                argument_index += 1;
                if !ceiling::is_exclusive_reference(program, parameter.type_reference) {
                    continue;
                }
                if matches!(
                    program.expression_table.expression(argument),
                    ExpressionNode::Borrow(_)
                ) {
                    // The direct borrow's own access above carries the exact
                    // lent referent and refines the path it contributed.
                    continue;
                }
                // A carried or returned exclusive value lends whatever storage
                // its referent names; the path records only the argument's own
                // spelling coarsened to its collection, so that spelling is the
                // coarsest contribution that must stay intact.
                let Some(actual) = canonical_place_from_expression_in_state(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    argument,
                ) else {
                    refinable = false;
                    break;
                };
                if !matches!(actual.root, facts::PlaceRoot::Symbol(_)) {
                    refinable = false;
                    break;
                }
                let mut coarse = actual;
                if let Some(index) = coarse
                    .segments
                    .iter()
                    .position(|segment| is_index_place_segment(*segment))
                {
                    coarse.segments.truncate(index);
                }
                match local_origins::rebase_local_write_places(
                    program,
                    caller_state_symbol,
                    borrow_call.statement_index,
                    coarse,
                    call_frames,
                ) {
                    Some(rebased) => {
                        for anchor in rebased {
                            let anchor = storage_normalized_place(
                                program,
                                caller_machine_symbol,
                                caller_state_symbol,
                                &anchor,
                            );
                            if !coarse_anchors.contains(&anchor) {
                                coarse_anchors.push(anchor);
                            }
                        }
                    }
                    None => {
                        refinable = false;
                        break;
                    }
                }
            }
        }
    } else if refinable {
        // Without a resolvable signature an exclusive actual's contribution
        // cannot be classified; keep every path coarse.
        refinable = false;
    }
    if refinable
        && borrow_call.has_receiver
        && let Some(receiver) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
    {
        match local_origins::rebase_local_write_places(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            receiver,
            call_frames,
        ) {
            Some(rebased) => {
                for receiver in rebased {
                    let receiver = storage_normalized_place(
                        program,
                        caller_machine_symbol,
                        caller_state_symbol,
                        &receiver,
                    );
                    if !coarse_anchors.contains(&receiver) {
                        coarse_anchors.push(receiver);
                    }
                }
            }
            None => refinable = false,
        }
    }
    (exclusive_referents, coarse_anchors, refinable)
}

fn is_index_place_segment(segment: facts::PlaceSegment) -> bool {
    matches!(
        segment,
        facts::PlaceSegment::FixedIndex { .. }
            | facts::PlaceSegment::FixedRange { .. }
            | facts::PlaceSegment::Index { .. }
    )
}

/// The machine-rooted storage identity used to compare frame-path places with
/// recorded access places: attached field roots rejoin their receiver storage
/// and the authored `self` parameter maps to the machine symbol, matching how
/// seeded field facts normalize.
fn storage_normalized_place(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    place: &CanonicalPlace,
) -> CanonicalPlace {
    let mut normalized = place.clone();
    normalize_attached_place_root(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        &mut normalized,
    );
    normalized.root = normalized_event_place_root(program, normalized.root);
    normalized
}

/// Whether `exact` names storage inside `coarse` whose first segment past
/// `coarse`'s own length is an index selector — the only extension a dotted
/// frame path could have coarsened away. An equal-length place (including a
/// whole-collection borrow or receiver) also counts: it contributed the path.
fn storage_extends_frame_place(
    program: &typed_trees::TypedTrees,
    exact: &CanonicalPlace,
    coarse: &CanonicalPlace,
) -> bool {
    exact.root == coarse.root
        && exact.segments.len() >= coarse.segments.len()
        && canonical_place_segments_may_overlap(
            program,
            &coarse.segments,
            &exact.segments[..coarse.segments.len()],
        )
        && (exact.segments.len() == coarse.segments.len()
            || is_index_place_segment(exact.segments[coarse.segments.len()]))
}

fn call_is_storage_free_asm_intrinsic(
    program: &typed_trees::TypedTrees,
    call: &BorrowCallFact,
) -> bool {
    // These canonical intrinsics affect machine services, not caller storage.
    // Input/read results are separate assignment writes in the typed tree.
    program
        .symbols
        .builtin_function_for_symbol(call.target_symbol)
        .is_some_and(symbols::BuiltinFunction::is_asm_intrinsic)
}

pub(crate) fn statement_mutated_place(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<CanonicalPlace> {
    let mut place = match statement {
        StatementNode::Assignment(assignment) => canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            assignment.target,
        )
        .or_else(|| {
            expression_root_symbol(assignment.target, &program.expression_table, machine_symbol)
                .and_then(canonical_place_from_symbol)
        }),
        _ => None,
    }?;
    normalize_write_only_range_place(program, state_symbol, &mut place);
    Some(place)
}

/// Storage writes for fact invalidation, including compiler-owned operations
/// on borrowed receivers. An unresolved origin requires full invalidation.
pub(crate) fn statement_storage_writes(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    if !matches!(
        statement,
        StatementNode::Assignment(_) | StatementNode::RootBinding(_)
    ) {
        return Some(Vec::new());
    }
    let places = if let StatementNode::RootBinding(binding) = statement {
        let place = canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            binding.receiver,
        )?;
        local_origins::rebase_local_write_places(
            program,
            state_symbol,
            statement_index,
            place,
            call_frames,
        )?
    } else {
        local_origins::assignment_storage_places(
            program,
            machine_symbol,
            state_symbol,
            statement_index,
            statement,
            call_frames,
        )?
    };
    // When the resolver cannot enumerate this state's local origins (a
    // reference local of finite candidate origins is such a state), borrow
    // exclusivity already forbids a second live alias of the written
    // storage, so the places stand without the closure.
    local_origins::close_storage_places_over_aliases_with_resolver(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        places.clone(),
        call_frames,
    )
    .or(Some(places))
}

/// Project a shared complete call frame into the exact caller storage namespace.
/// Coarse selectors remain conservative writes; they are not value provenance.
pub(crate) fn frame_storage_writes(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    frame: &facts::NormalizedWriteFrame,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let state = find_state(program, state_symbol)?;
    let mut places = Vec::new();
    for path in frame.complete_paths()? {
        let source = local_origins::place_from_origin_path(program, state, statement_index, path)?;
        for place in local_origins::rebase_local_write_places(
            program,
            state_symbol,
            statement_index,
            source,
            call_frames,
        )? {
            if !places.contains(&place) {
                places.push(place);
            }
        }
    }
    local_origins::close_storage_places_over_aliases_with_resolver(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        places,
        call_frames,
    )
}

fn normalize_write_only_range_place(
    program: &typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
    place: &mut CanonicalPlace,
) {
    // Keep ordinary borrow selectors expression-backed for certificate replay.
    // Only an admitted write-only mutation may collapse immutable copy bounds
    // into the exact caller-visible range footprint.
    let facts::PlaceRoot::Symbol(root_symbol) = place.root else {
        return;
    };
    let Some(state) = find_state(program, state_symbol) else {
        return;
    };
    let root_is_write_only = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == root_symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .find_map(|statement| match statement {
                    StatementNode::LocalData(local) if local.symbol == root_symbol => {
                        Some(local.type_reference)
                    }
                    _ => None,
                })
        })
        .is_some_and(|type_reference| {
            matches!(
                program.type_reference_table.type_reference(type_reference),
                typed_trees::types::TypeReferenceNode::Reference {
                    access: language_semantics::ReferenceAccess::WriteOnly,
                    ..
                }
            )
        });
    if !root_is_write_only {
        return;
    }

    for segment in &mut place.segments {
        let facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let ExpressionNode::Range(range) = program.expression_table.expression(expression) else {
            continue;
        };
        let start = if range.start.is_valid() {
            validation::normalize_immutable_integer_bound_to_usize(program, range.start)
        } else {
            Some(0)
        };
        let end = if !range.end.is_valid() {
            None
        } else {
            validation::normalize_immutable_integer_bound_to_usize(program, range.end).and_then(
                |end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                },
            )
        };
        if let (Some(start), Some(end)) = (start, end) {
            *segment = facts::PlaceSegment::FixedRange { start, end };
        }
    }
}
