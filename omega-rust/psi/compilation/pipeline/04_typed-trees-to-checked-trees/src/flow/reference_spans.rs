//! Append, filter, and retain over the flow facts' reference spans.
//!
//! Semantic-context refs, constraint refs, and place segments all live as
//! arena spans on the flow facts. Construction appends to a tail span in
//! place and copies only when another append has moved the tail, so the
//! rules for growing and narrowing one of these lists live here instead of
//! being restated at each producing site.
use checked_trees::{FlowConstraintKind, FlowConstraintRef, FlowSemanticContextRef};
use facts::{FactPlan, ProgramPoint};

#[cfg(test)]
mod tests;

/// Keep exact incoming reference membership in the skipped path's order.
/// Scratch keys never become published identities: they include the complete
/// generational handle and retain the reference kind for heterogeneous debts.
/// Duplicate skipped references survive, just as with a per-row `contains`.
fn intersect_references_in_order<Reference: Copy, Identity: Ord>(
    skipped: &[Reference],
    evaluated: &[Reference],
    identity: impl Fn(&Reference) -> Identity,
) -> Vec<Reference> {
    let incoming = sorted_reference_identities(evaluated, &identity);
    skipped
        .iter()
        .filter(|reference| incoming.binary_search(&identity(reference)).is_ok())
        .copied()
        .collect()
}

fn sorted_reference_identities<Reference, Identity: Ord>(
    references: &[Reference],
    identity: impl Fn(&Reference) -> Identity,
) -> Vec<Identity> {
    let mut identities: Vec<_> = references.iter().map(identity).collect();
    identities.sort_unstable();
    identities.dedup();
    identities
}

fn semantic_context_identity(context: facts::FactContextHandle) -> (u32, u32) {
    (context.arena_index(), context.generation())
}

/// An immutable scratch index of exact live context handles, not evidence.
/// Repeated qualification queries share this index for one completed frontier;
/// it never survives a write, a new flow snapshot or a statement boundary.
pub(crate) struct SemanticContextMembership {
    identities: Vec<(u32, u32)>,
}

impl SemanticContextMembership {
    pub(crate) fn new(references: &[FlowSemanticContextRef]) -> Self {
        Self {
            identities: sorted_reference_identities(references, |reference| {
                semantic_context_identity(reference.context)
            }),
        }
    }

    pub(crate) fn contains(&self, context: facts::FactContextHandle) -> bool {
        self.identities
            .binary_search(&semantic_context_identity(context))
            .is_ok()
    }
}

pub(crate) fn intersect_semantic_contexts_in_order(
    skipped: &[FlowSemanticContextRef],
    evaluated: &[FlowSemanticContextRef],
) -> Vec<FlowSemanticContextRef> {
    let incoming = SemanticContextMembership::new(evaluated);
    skipped
        .iter()
        .filter(|reference| incoming.contains(reference.context))
        .copied()
        .collect()
}

fn constraint_reference_identity(reference: &FlowConstraintRef) -> (u8, u32, u32) {
    // Handles have exactly arena_index and generation; page identity is carried
    // by the generation. The closed variant tag prevents different typed arenas
    // with the same handle bits from naming the same constraint.
    fn identity<T>(kind: u8, handle: arena::Handle<T>) -> (u8, u32, u32) {
        (kind, handle.arena_index(), handle.generation())
    }
    match reference.kind {
        FlowConstraintKind::Unknown => (0, 0, 0),
        FlowConstraintKind::SemanticContext { context } => identity(1, context),
        FlowConstraintKind::BorrowState { state } => identity(2, state),
        FlowConstraintKind::BorrowCall { call } => identity(3, call),
        FlowConstraintKind::BorrowWritableRoot { root } => identity(4, root),
        FlowConstraintKind::BorrowAccess { access } => identity(5, access),
        FlowConstraintKind::BorrowLoan { loan } => identity(6, loan),
        FlowConstraintKind::BorrowIncomingReference { identity: incoming } => identity(7, incoming),
    }
}

/// Semantic constraints meet; loan and incoming-reference debts join.
/// Existing intersection duplicates and order remain unchanged. Additional
/// debts appear once, in first encounter order over skipped then evaluated.
/// Sorted scratch membership avoids scanning either growing output per debt.
pub(crate) fn meet_constraint_references_in_order(
    skipped: &[FlowConstraintRef],
    evaluated: &[FlowConstraintRef],
) -> Vec<FlowConstraintRef> {
    let mut retained =
        intersect_references_in_order(skipped, evaluated, constraint_reference_identity);
    let mut retained_identities: Vec<_> =
        retained.iter().map(constraint_reference_identity).collect();
    retained_identities.sort_unstable();
    retained_identities.dedup();
    let mut additions: Vec<_> = skipped
        .iter()
        .chain(evaluated)
        .enumerate()
        .filter(|(_, reference)| {
            matches!(
                reference.kind,
                FlowConstraintKind::BorrowLoan { .. }
                    | FlowConstraintKind::BorrowIncomingReference { .. }
            )
        })
        .filter_map(|(order, reference)| {
            let identity = constraint_reference_identity(reference);
            retained_identities
                .binary_search(&identity)
                .is_err()
                .then_some((identity, order, *reference))
        })
        .collect();
    // Include encounter order in the sort so the earliest occurrence survives
    // deduplication even though the scratch sort itself is unstable.
    additions.sort_unstable_by_key(|(identity, order, _)| (*identity, *order));
    additions.dedup_by_key(|(identity, _, _)| *identity);
    additions.sort_unstable_by_key(|(_, order, _)| *order);
    retained.extend(additions.into_iter().map(|(_, _, reference)| reference));
    retained
}

pub(crate) fn retained_flow_contexts(
    semantic_context_refs: &arena::PagedArenaSet<FlowSemanticContextRef>,
    source: arena::HandleSpan<FlowSemanticContextRef>,
) -> arena::HandleSpan<FlowSemanticContextRef> {
    if semantic_context_refs.span_or_empty(source).is_empty() {
        arena::HandleSpan::empty()
    } else {
        source
    }
}

// Reference rows are append-only during flow construction, so lists are shared
// snapshots (the public evidence arenas can still be mutated afterward).
// Extending a non-tail snapshot first
// copies its rows; extending a tail only changes the caller's span length.
// Callers supply a validated selection or a list built here. Revalidating its
// entire prefix on every append would turn list construction quadratic.
pub(crate) fn append_flow_reference<T: Copy + Default>(
    references: &mut arena::PagedArenaSet<T>,
    span: &mut arena::HandleSpan<T>,
    reference: T,
) {
    // A span off the current page or behind its tail is first copied there.
    references.append_to_span(span, reference);
}

pub(crate) fn appended_span_since<T: Default>(
    arena: &arena::PagedArenaSet<T>,
    start: arena::PagedArenaFrontier<T>,
) -> arena::HandleSpan<T> {
    arena.span_since(start)
}

pub(crate) fn append_place_segments(
    segments_arena: &mut arena::PagedArenaSet<facts::PlaceSegment>,
    segments: &[facts::PlaceSegment],
) -> arena::HandleSpan<facts::PlaceSegment> {
    let start_len = segments_arena.frontier();
    for segment in segments {
        segments_arena.append(*segment);
    }
    appended_span_since(segments_arena, start_len)
}

pub(crate) fn append_flow_contexts_for_points(
    semantic: &FactPlan,
    semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
    refs: &mut arena::HandleSpan<FlowSemanticContextRef>,
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    constraints: &mut arena::HandleSpan<FlowConstraintRef>,
    points: &[ProgramPoint],
) {
    append_flow_contexts(
        points
            .iter()
            .flat_map(|point| semantic.context_handles_at_point(*point)),
        semantic_context_refs,
        refs,
        constraint_refs,
        constraints,
    );
}

pub(crate) fn append_flow_contexts(
    contexts: impl Iterator<Item = facts::FactContextHandle>,
    semantic_context_refs: &mut arena::PagedArenaSet<FlowSemanticContextRef>,
    refs: &mut arena::HandleSpan<FlowSemanticContextRef>,
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    constraints: &mut arena::HandleSpan<FlowConstraintRef>,
) {
    // Both lists describe the same selected contexts. Keep their arena order.
    for context in contexts {
        append_flow_reference(
            semantic_context_refs,
            refs,
            FlowSemanticContextRef { context },
        );
        append_constraint_ref(
            constraint_refs,
            constraints,
            FlowConstraintKind::SemanticContext { context },
        );
    }
}

pub(crate) fn retained_constraint_refs(
    constraint_refs: &arena::PagedArenaSet<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
) -> arena::HandleSpan<FlowConstraintRef> {
    if constraint_refs.span_or_empty(source).is_empty() {
        arena::HandleSpan::empty()
    } else {
        source
    }
}

/// Publish one semantic context to both the context-ref and constraint-ref
/// tails — the two evidence lists a flow site always extends together.
pub(crate) fn append_semantic_context(
    semantic: &mut FactPlan,
    build: &mut super::context::FlowBuildContext<'_>,
    exit: &mut super::call_phases::CallFlowContexts,
    point: ProgramPoint,
    refs: arena::HandleSpan<facts::FactRef>,
) {
    let context = semantic.append_context(point, refs);
    append_flow_reference(
        &mut build.contexts.semantic_context_refs,
        &mut exit.contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut build.contexts.constraint_refs,
        &mut exit.constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

pub(crate) fn append_constraint_ref(
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    refs: &mut arena::HandleSpan<FlowConstraintRef>,
    kind: FlowConstraintKind,
) {
    append_flow_reference(constraint_refs, refs, FlowConstraintRef { kind });
}

pub(crate) fn project_constraint_refs_to_active_contexts(
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    active_contexts: arena::HandleSpan<FlowSemanticContextRef>,
    semantic_context_refs: &arena::PagedArenaSet<FlowSemanticContextRef>,
) -> arena::HandleSpan<FlowConstraintRef> {
    // Both lists seed from the whole Global/Machine declaration groups, so a
    // scan of the active list per constraint is quadratic in the program's
    // declaration count for every state on every convergence pass. Read the
    // active list once and probe it per constraint. Arena positions are local
    // to a page: declaration and machine contexts can share a slot while both
    // remain active. Select the generation's dense row before its slot, keeping
    // the complete handle as the membership identity.
    let mut active = std::collections::BTreeMap::new();
    for reference in semantic_context_refs.span_or_empty(active_contexts) {
        let position = reference.context.arena_index() as usize;
        let page = active
            .entry(reference.context.generation())
            .or_insert_with(Vec::new);
        if page.len() <= position {
            page.resize(position + 1, facts::FactContextHandle::default());
        }
        page[position] = reference.context;
    }
    filter_constraint_refs(
        constraint_refs,
        source,
        |constraint_ref| match constraint_ref.kind {
            FlowConstraintKind::SemanticContext { context } => active
                .get(&context.generation())
                .and_then(|page| page.get(context.arena_index() as usize))
                .is_some_and(|known| *known == context),
            FlowConstraintKind::Unknown | checked_trees::borrow_resource_pattern!(..) => true,
        },
    )
}

pub(crate) fn filter_constraint_refs(
    constraint_refs: &mut arena::PagedArenaSet<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    keep: impl FnMut(FlowConstraintRef) -> bool,
) -> arena::HandleSpan<FlowConstraintRef> {
    filter_flow_references(constraint_refs, source, keep)
}

pub(crate) fn filter_flow_references<T: Copy + Default>(
    references: &mut arena::PagedArenaSet<T>,
    source: arena::HandleSpan<T>,
    mut keep: impl FnMut(T) -> bool,
) -> arena::HandleSpan<T> {
    let mut filtered = arena::HandleSpan::empty();
    // Validate the entire source before appending: a stale member invalidates
    // the whole list. Appends cannot change its entries or generations, and
    // reading one handle at a time needs no temporary snapshot across growth.
    let mut materialized = false;
    for source_handle in source.handles() {
        let reference = *references.get(source_handle);
        if !keep(reference) {
            continue;
        }
        if filtered.is_empty() {
            filtered = arena::HandleSpan::from_parts(source_handle, 1);
        } else if !materialized
            && filtered.start().arena_index() + filtered.count() == source_handle.arena_index()
        {
            filtered.push_contiguous(source_handle);
        } else {
            if !materialized {
                filtered = references.copy_span_pair(filtered, arena::HandleSpan::empty());
                materialized = true;
            }
            references.append_to_span(&mut filtered, reference);
        }
    }

    filtered
}
