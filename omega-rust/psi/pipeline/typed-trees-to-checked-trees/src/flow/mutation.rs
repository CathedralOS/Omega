use super::*;
use crate::lookup::expression_root_symbol;
mod local_origins;
mod receiver;
mod summary;

pub(super) use local_origins::close_storage_places_over_aliases;
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
pub(crate) fn call_mutated_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &mut StateMutationSummaryCache,
) -> Option<Vec<CanonicalPlace>> {
    call_write_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        WritePlaceNamespace::Storage,
    )
}

/// The access route retains the local loan owner; storage rebasing must not
/// turn a write through that loan into an independent write to its referent.
pub(crate) fn call_write_accesses(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &mut StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    call_write_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        WritePlaceNamespace::AccessRoute,
    )
    .expect("access-route projection always retains the ownership fallback")
}

fn call_write_places(
    program: &typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &mut StateMutationSummaryCache,
    namespace: WritePlaceNamespace,
) -> Option<Vec<CanonicalPlace>> {
    let summarized_places = instantiate_known_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow,
        borrow_call,
        state_mutation_summaries,
        namespace,
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
                borrow_call,
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
    borrow_call: &BorrowCallFact,
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
    let resolver = validation::CallFrameResolver::new(program)?;
    let frame = match &site {
        CallSite::Statement(call) => resolver.may_write_frame(machine, call),
        CallSite::Expression { expression, .. } => {
            resolver.expression_write_frame(machine, *expression)
        }
        CallSite::TransitionNamed { .. } => return None,
    };
    let mut places = Vec::new();
    for path in frame.complete_paths()? {
        let source = local_origins::place_from_origin_path(
            program,
            state,
            borrow_call.statement_index,
            path,
        )?;
        for place in local_origins::rebase_local_write_places(
            program,
            caller_state_symbol,
            borrow_call.statement_index,
            source,
        )? {
            if !places.contains(&place) {
                places.push(place);
            }
        }
    }
    Some(places)
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

/// Storage writes for fact invalidation. Non-assignment statements have no
/// direct store; an unresolved assignment origin requires full invalidation.
pub(crate) fn statement_storage_writes(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<Vec<CanonicalPlace>> {
    if !matches!(statement, StatementNode::Assignment(_)) {
        return Some(Vec::new());
    }
    let places = local_origins::assignment_storage_places(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        statement,
    )?;
    local_origins::close_storage_places_over_aliases(
        program,
        machine_symbol,
        state_symbol,
        statement_index,
        places,
    )
}

/// Project a shared complete call frame into the exact caller storage namespace.
/// Coarse selectors remain conservative writes; they are not value provenance.
pub(crate) fn frame_storage_writes(
    program: &typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    frame: &facts::NormalizedWriteFrame,
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
