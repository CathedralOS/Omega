//! Storage places a statement or call writes, as `CanonicalPlace`s:
//! `statement_storage_writes`, `call_mutated_places` (which falls back to the
//! declared signature ceiling in `ceiling`), and `frame_storage_writes`. When a
//! statement's or call's write set is `None`, flow drops every active context.

use validation::CallSite;
use validation::CanonicalPlace;
use validation::call_site_argument_expressions;
use validation::canonical_place_from_expression_in_state;
use validation::expression_root_symbol;
use validation::find_call_site;

use crate::flow::canonical_place_from_symbol;
use crate::flow::canonical_place_segments_may_overlap;

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
pub(crate) use local_origins::close_storage_places_over_aliases;
pub(crate) use local_origins::place_from_origin_path;
pub(crate) use local_origins::rebase_exact_local_place;
pub(crate) use local_origins::{OriginPrecision, origin_place, sole_local_referent_origin};
pub(in crate::flow) use receiver::local_jump_data_argument;
pub(crate) use receiver::{
    call_receiver_is_mutable, call_receiver_mutated_place, canonical_receiver_place_for_call_site,
};
pub(in crate::flow) use summary::instantiate_call_relative_places;
use summary::instantiate_known_call_mutation_summary_places;
pub(crate) use summary::{StateMutationSummaryCache, state_storage_writes};

#[derive(Clone, Copy)]
pub(super) enum WritePlaceNamespace {
    /// Preserve the caller's reference binding through which access occurs.
    AccessRoute,
    /// Rebase reference bindings to the storage whose facts may be invalidated.
    Storage,
}

/// One possible write retains the callee storage path that supplied it.
/// Equal caller footprints reached through different formals remain distinct:
/// one writer's restored contract cannot excuse another writer's corruption.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::flow) struct CallMutationWrite {
    pub place: CanonicalPlace,
    pub source: Option<CanonicalPlace>,
}

/// Caller storage footprint. `None` requires full invalidation; an empty
/// complete footprint preserves facts. Neither may be represented by an
/// unresolved reference-binding root.
/// Reuse the resolver prepared for this immutable program; local origins still
/// resolve at the exact caller prefix, not from an earlier query's facts.
pub(crate) fn call_mutated_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    call_mutation_writes(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        call_frames,
    )
    .map(|writes| writes.into_iter().map(|write| write.place).collect())
}

pub(in crate::flow) fn call_mutation_writes(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CallMutationWrite>> {
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
    // Reserved byte predicates observe storage but cannot write it. Recognize
    // the resolved operation, not its spelling: a declared machine with the
    // same name retains its ordinary frame. Operand calls still contribute
    // their own effects in evaluation order before this invocation.
    if let CallSite::Expression { expression, .. } = site
        && typed_trees::byte_predicates::byte_predicate_call(program, expression).is_some()
    {
        return Some(Vec::new());
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
        None,
    )
    .or_else(|| {
        ceiling::signature_ceiling_places(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            Some(borrow),
            borrow_call,
            call_frames,
        )
        .map(unattributed_writes)
    })
}

/// The access route retains the local loan owner; storage rebasing must not
/// turn a write through that loan into an independent write to its referent.
/// Only exclusive writes belong here. Shared atomic operations keep their
/// ordinary call-access checks and remain writes in `call_mutated_places`,
/// but do not freeze other shared loans as an ordinary store would.
/// A captured loan limits a local jump to the states that still carry it;
/// ordinary calls retain it through their complete run. The caller must first
/// authenticate the source binding's closure; this spatial query grants no
/// lifetime authority. Without a loan, callers
/// inspect the complete write set (for example, restored-use certificates).
pub(crate) fn call_write_accesses(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    loan: Option<&CanonicalPlace>,
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
        loan,
    )
    .expect("access-route projection always retains the ownership fallback")
    .into_iter()
    .map(|write| write.place)
    .collect()
}

/// The subset of `call_write_accesses` a call writes through its implicit
/// receiver: the callee's complete exclusive writes reached through `self`, or
/// the whole receiver place when no complete summary is known. Writes through
/// explicit `&mut` arguments are excluded; argument checks own those.
pub(crate) fn call_receiver_write_accesses(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    loan: &CanonicalPlace,
) -> Vec<CanonicalPlace> {
    summary::instantiate_known_call_receiver_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        loan,
    )
    .or_else(|| {
        call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        .map(|place| vec![place])
    })
    .unwrap_or_default()
}

fn call_write_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
    loan: Option<&CanonicalPlace>,
) -> Option<Vec<CallMutationWrite>> {
    let summarized_places = instantiate_known_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        namespace,
        call_frames,
        loan,
    );
    if let Some(summarized_places) = summarized_places {
        return Some(summarized_places);
    }
    let mut places = Vec::new();
    {
        for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
            if access.kind.is_exclusive() && access.root.is_valid() {
                let mut place = CanonicalPlace {
                    root: access.root,
                    segments: Vec::new(),
                };
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
        ) && let Some(target_state) = program.state_by_symbol(borrow_call.target_symbol)
        {
            let mut argument_index = 0usize;
            for parameter in program.state_parameters(target_state) {
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

    if borrow_call.has_receiver
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

    if matches!(namespace, WritePlaceNamespace::Storage) {
        let boundary_target = program.traits().iter().any(|definition| {
            definition.is_boundary
                && program
                    .trait_machine_signature_by_symbol(definition, borrow_call.target_symbol)
                    .is_some()
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
            )
            .map(unattributed_writes);
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
        Some(unattributed_writes(storage))
    } else {
        Some(unattributed_writes(places))
    }
}

fn unattributed_writes(places: Vec<CanonicalPlace>) -> Vec<CallMutationWrite> {
    places
        .into_iter()
        .map(|place| CallMutationWrite {
            place,
            source: None,
        })
        .collect()
}

fn shared_call_storage_places(
    program: &dyn ::validation::CheckingInput,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let machine = program.machine_by_symbol(caller_machine_symbol)?;
    let state = program.state_by_symbol(caller_state_symbol)?;
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
            let identity = coarse.clone();
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
    program: &dyn ::validation::CheckingInput,
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
        if !access.root.is_valid() {
            refinable = false;
            break;
        }
        let mut place = CanonicalPlace {
            root: access.root,
            segments: Vec::new(),
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
            if !exclusive_referents.contains(&rebased) {
                exclusive_referents.push(rebased);
            }
        }
    }
    if refinable
        && let Some(parameters) =
            validation::call_target_parameters(program, borrow_call.target_symbol)
    {
        let arguments = call_site_argument_expressions(program, site);
        if parameters.iter().count() != arguments.len() {
            refinable = false;
        } else {
            let mut argument_index = 0usize;
            for parameter in parameters {
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

/// Whether `exact` names storage inside `coarse` whose first segment past
/// `coarse`'s own length is an index selector — the only extension a dotted
/// frame path could have coarsened away. An equal-length place (including a
/// whole-collection borrow or receiver) also counts: it contributed the path.
fn storage_extends_frame_place(
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    program: &dyn ::validation::CheckingInput,
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
    let places = if let Some(binding) = statement.as_root_binding() {
        super::reference_expression_storage_places(
            program,
            state_symbol,
            statement_index,
            binding.receiver,
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
    local_origins::close_storage_places_over_aliases(
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
    program: &dyn ::validation::CheckingInput,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    frame: &facts::NormalizedWriteFrame,
    call_frames: Option<&validation::CallFrameResolver<'_>>,
) -> Option<Vec<CanonicalPlace>> {
    let state = program.state_by_symbol(state_symbol)?;
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
    local_origins::close_storage_places_over_aliases(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        places,
        call_frames,
    )
}

fn normalize_write_only_range_place(
    program: &dyn ::validation::CheckingInput,
    state_symbol: SymbolHandle,
    place: &mut CanonicalPlace,
) {
    // Keep ordinary borrow selectors expression-backed for certificate replay.
    // Only an admitted write-only mutation may collapse immutable copy bounds
    // into the exact caller-visible range footprint.
    let Some(root_symbol) = place.root.as_symbol() else {
        return;
    };
    let Some(state) = program.state_by_symbol(state_symbol) else {
        return;
    };
    let root_is_write_only = program
        .state_parameter_by_symbol(state, root_symbol)
        .map(|parameter| parameter.type_reference)
        .or_else(|| {
            program
                .state_local_declaration(state, root_symbol)
                .map(|(_, local)| local.type_reference)
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

    // The bound index scans the whole program once; only Range selectors
    // need it, so build it on the first one rather than for every place.
    let mut bound_lookup = None;
    for segment in &mut place.segments {
        let facts::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let Some(range) = program.expression_table.range_node(expression) else {
            continue;
        };
        let bound_lookup =
            bound_lookup.get_or_insert_with(|| validation::ImmutableBoundLookup::new(program));
        let start = if range.start.is_valid() {
            validation::normalize_immutable_integer_bound_to_usize(
                program,
                bound_lookup,
                range.start,
            )
        } else {
            Some(0)
        };
        let end = if !range.end.is_valid() {
            None
        } else {
            validation::normalize_immutable_integer_bound_to_usize(program, bound_lookup, range.end)
                .and_then(|end| {
                    if range.end_inclusive {
                        end.checked_add(1)
                    } else {
                        Some(end)
                    }
                })
        };
        if let (Some(start), Some(end)) = (start, end) {
            *segment = facts::PlaceSegment::FixedRange { start, end };
        }
    }
}
