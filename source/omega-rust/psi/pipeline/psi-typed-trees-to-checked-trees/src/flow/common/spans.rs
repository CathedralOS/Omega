use super::*;

pub(crate) fn clone_flow_contexts(
    semantic_context_refs: &mut psi_arena::Arena<FlowSemanticContextRef>,
    source: psi_arena::HandleSpan<FlowSemanticContextRef>,
) -> psi_arena::HandleSpan<FlowSemanticContextRef> {
    let mut cloned = psi_arena::HandleSpan::empty();
    let copied: Vec<_> = semantic_context_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();
    for context_ref in copied {
        semantic_context_refs.append_to_span(&mut cloned, context_ref);
    }
    cloned
}

pub(crate) fn appended_span_since<T: Clone + Default + PartialEq + Eq>(
    arena: &psi_arena::Arena<T>,
    start_len: usize,
) -> psi_arena::HandleSpan<T> {
    let appended = arena.len().saturating_sub(start_len);
    if appended == 0 {
        psi_arena::HandleSpan::empty()
    } else {
        psi_arena::HandleSpan::from_parts(
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
    segments_arena: &mut psi_arena::Arena<psi_facts::PlaceSegment>,
    segments: &[psi_facts::PlaceSegment],
) -> psi_arena::HandleSpan<psi_facts::PlaceSegment> {
    let start_len = segments_arena.len();
    for segment in segments {
        segments_arena.append(*segment);
    }
    appended_span_since(segments_arena, start_len)
}

pub(crate) fn append_flow_contexts_for_points(
    semantic: &FactPlan,
    semantic_context_refs: &mut psi_arena::Arena<FlowSemanticContextRef>,
    refs: &mut psi_arena::HandleSpan<FlowSemanticContextRef>,
    points: &[ProgramPoint],
) {
    for point in points {
        for context in semantic.context_handles_at_point(*point) {
            semantic_context_refs.append_to_span(refs, FlowSemanticContextRef { context });
        }
    }
}

pub(crate) fn clone_constraint_refs(
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    source: psi_arena::HandleSpan<FlowConstraintRef>,
) -> psi_arena::HandleSpan<FlowConstraintRef> {
    let mut cloned = psi_arena::HandleSpan::empty();
    let copied: Vec<_> = constraint_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();
    for constraint_ref in copied {
        constraint_refs.append_to_span(&mut cloned, constraint_ref);
    }
    cloned
}

pub(crate) fn append_constraint_ref(
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    refs: &mut psi_arena::HandleSpan<FlowConstraintRef>,
    kind: FlowConstraintKind,
) {
    constraint_refs.append_to_span(refs, FlowConstraintRef { kind });
}

pub(crate) fn append_semantic_constraints_for_points(
    semantic: &FactPlan,
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    refs: &mut psi_arena::HandleSpan<FlowConstraintRef>,
    points: &[ProgramPoint],
) {
    for point in points {
        for context in semantic.context_handles_at_point(*point) {
            append_constraint_ref(
                constraint_refs,
                refs,
                FlowConstraintKind::SemanticContext { context },
            );
        }
    }
}

pub(crate) fn project_constraint_refs_to_active_contexts(
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    source: psi_arena::HandleSpan<FlowConstraintRef>,
    active_contexts: psi_arena::HandleSpan<FlowSemanticContextRef>,
    semantic_context_refs: &psi_arena::Arena<FlowSemanticContextRef>,
) -> psi_arena::HandleSpan<FlowConstraintRef> {
    let active_context_handles: Vec<_> = semantic_context_refs
        .span_or_empty(active_contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect();
    let mut projected = psi_arena::HandleSpan::empty();
    let copied: Vec<_> = constraint_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();

    for constraint_ref in copied {
        let keep = match constraint_ref.kind {
            FlowConstraintKind::SemanticContext { context } => {
                active_context_handles.contains(&context)
            }
            FlowConstraintKind::Unknown
            | FlowConstraintKind::BorrowState { .. }
            | FlowConstraintKind::BorrowCall { .. }
            | FlowConstraintKind::BorrowWritableRoot { .. }
            | FlowConstraintKind::BorrowAccess { .. }
            | FlowConstraintKind::BorrowLoan { .. } => true,
        };

        if keep {
            constraint_refs.append_to_span(&mut projected, constraint_ref);
        }
    }

    projected
}

pub(crate) fn filter_constraint_refs(
    constraint_refs: &mut psi_arena::Arena<FlowConstraintRef>,
    source: psi_arena::HandleSpan<FlowConstraintRef>,
    mut keep: impl FnMut(FlowConstraintRef) -> bool,
) -> psi_arena::HandleSpan<FlowConstraintRef> {
    let mut filtered = psi_arena::HandleSpan::empty();
    let copied: Vec<_> = constraint_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();

    for constraint_ref in copied {
        if keep(constraint_ref) {
            constraint_refs.append_to_span(&mut filtered, constraint_ref);
        }
    }

    filtered
}
