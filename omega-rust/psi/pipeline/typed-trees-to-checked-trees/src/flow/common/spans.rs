use super::*;

#[cfg(test)]
mod tests;

pub(crate) fn retained_flow_contexts(
    semantic_context_refs: &arena::Arena<FlowSemanticContextRef>,
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
    references: &mut arena::Arena<T>,
    span: &mut arena::HandleSpan<T>,
    reference: T,
) {
    if !span.is_empty()
        && (span.start().generation() != 1
            || u64::from(span.start().arena_index()) + u64::from(span.count())
                != references.storage_slice().len() as u64 + 1)
    {
        *span = references.copy_span_pair(*span, arena::HandleSpan::empty());
    }
    references.append_to_span(span, reference);
}

pub(crate) fn appended_span_since<T: Clone + Default + PartialEq + Eq>(
    arena: &arena::Arena<T>,
    start_len: usize,
) -> arena::HandleSpan<T> {
    let appended = arena.len().saturating_sub(start_len);
    if appended == 0 {
        arena::HandleSpan::empty()
    } else {
        arena::HandleSpan::from_parts(
            Handle::from_arena_index(
                start_len
                    .checked_add(1)
                    .and_then(|index| index.try_into().ok())
                    .unwrap(),
            ),
            appended.try_into().unwrap(),
        )
    }
}

pub(crate) fn append_place_segments(
    segments_arena: &mut arena::Arena<facts::PlaceSegment>,
    segments: &[facts::PlaceSegment],
) -> arena::HandleSpan<facts::PlaceSegment> {
    let start_len = segments_arena.len();
    for segment in segments {
        segments_arena.append(*segment);
    }
    appended_span_since(segments_arena, start_len)
}

pub(crate) fn append_flow_contexts_for_points(
    semantic: &FactPlan,
    semantic_context_refs: &mut arena::Arena<FlowSemanticContextRef>,
    refs: &mut arena::HandleSpan<FlowSemanticContextRef>,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
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
    semantic_context_refs: &mut arena::Arena<FlowSemanticContextRef>,
    refs: &mut arena::HandleSpan<FlowSemanticContextRef>,
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
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
    constraint_refs: &arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
) -> arena::HandleSpan<FlowConstraintRef> {
    if constraint_refs.span_or_empty(source).is_empty() {
        arena::HandleSpan::empty()
    } else {
        source
    }
}

pub(crate) fn append_constraint_ref(
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    refs: &mut arena::HandleSpan<FlowConstraintRef>,
    kind: FlowConstraintKind,
) {
    append_flow_reference(constraint_refs, refs, FlowConstraintRef { kind });
}

pub(crate) fn project_constraint_refs_to_active_contexts(
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    active_contexts: arena::HandleSpan<FlowSemanticContextRef>,
    semantic_context_refs: &arena::Arena<FlowSemanticContextRef>,
) -> arena::HandleSpan<FlowConstraintRef> {
    let active_contexts = semantic_context_refs.span_or_empty(active_contexts);
    filter_constraint_refs(
        constraint_refs,
        source,
        |constraint_ref| match constraint_ref.kind {
            FlowConstraintKind::SemanticContext { context } => active_contexts
                .iter()
                .any(|active| active.context == context),
            FlowConstraintKind::Unknown
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. }
            | FlowConstraintKind::BorrowLoan { .. } => true,
        },
    )
}

pub(crate) fn filter_constraint_refs(
    constraint_refs: &mut arena::Arena<FlowConstraintRef>,
    source: arena::HandleSpan<FlowConstraintRef>,
    keep: impl FnMut(FlowConstraintRef) -> bool,
) -> arena::HandleSpan<FlowConstraintRef> {
    filter_flow_references(constraint_refs, source, keep)
}

pub(crate) fn filter_flow_references<T: Copy + Default>(
    references: &mut arena::Arena<T>,
    source: arena::HandleSpan<T>,
    mut keep: impl FnMut(T) -> bool,
) -> arena::HandleSpan<T> {
    let mut filtered = arena::HandleSpan::empty();
    // Validate the entire source before appending: a stale member invalidates
    // the whole list. Appends cannot change its entries or generations, and
    // reading one handle at a time needs no temporary snapshot across growth.
    let source_count = references.span_or_empty(source).len() as u32;
    let mut materialized = false;
    for source_offset in 0..source_count {
        let source_handle = Handle::from_parts(
            source.start().arena_index() + source_offset,
            source.start().generation(),
        );
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
