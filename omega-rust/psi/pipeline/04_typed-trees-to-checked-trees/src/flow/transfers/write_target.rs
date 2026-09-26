//! Where one statement writes: the place a `let` binds or an assignment
//! stores to, the value's source, and the finite candidates a write through
//! a reference local may reach. A selector whose value is already pinned
//! narrows to the one element it names.

use super::{
    CanonicalPlace, ExpressionHandle, FactPlan, FlowBuildContext, PlaceHandle, PlaceRoot,
    StatementNode, SymbolHandle, contextual_expression_place,
};

/// The place one statement writes and the value it writes there.
pub(super) struct StatementWrite {
    pub(super) target_place: PlaceHandle,
    pub(super) source_expression: ExpressionHandle,
    pub(super) source_place: Option<PlaceHandle>,
    /// The finite origins a write through a reference local may reach.
    pub(super) candidate_targets: Vec<CanonicalPlace>,
}

pub(super) fn statement_write(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
) -> Option<StatementWrite> {
    // An assignment through a reference local of finite candidate origins
    // (flow/reference_places) rewrites exactly one candidate with a value the
    // write checker proved in the destination's declared domains; every
    // other candidate keeps its previous contents. Each candidate therefore
    // holds a declared domain after the write exactly when it held it
    // before, and those rows are re-established below.
    let mut candidate_targets: Vec<CanonicalPlace> = Vec::new();
    let (target_place, source_expression, source_place) = match statement {
        StatementNode::RootBinding(_) | StatementNode::AssemblyFact(_) => return None,
        StatementNode::LocalData(local_data) => (
            semantic.append_symbol_place(local_data.symbol),
            local_data.initial_value,
            contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                local_data.initial_value,
            ),
        ),
        StatementNode::Assignment(assignment) => {
            // A write through a local `&mut` alias establishes its facts on
            // the exact storage it aliases -- the same place invalidation
            // retires -- so `alias.out = "XXX"` and `label_alias = "hello"`
            // close the window on the aliased field. A local binding
            // replacement rebinds the reference itself and an ambiguous
            // origin proves nothing exact, so both keep the alias place.
            let target_place = build
                .canonical_place_at(program, state_symbol, statement_index, assignment.target)
                .map(|canonical| {
                    let mut owned_frames = None;
                    let writes_through_alias = crate::flow::shared_call_frames_or(
                        build.call_frames,
                        program,
                        &mut owned_frames,
                    )
                    .zip(
                        build
                            .machine_index(program, machine_symbol)
                            .map(|index| &program.machines()[index]),
                    )
                    .and_then(|(resolver, machine)| {
                        resolver.assignment_write_target(machine, statement)
                    })
                    // An unclassified target is a write through a
                    // reference local the resolver has no origin for;
                    // only a local binding replacement keeps the alias.
                    .is_none_or(|target| {
                        matches!(
                            target,
                            crate::validation::AssignmentWriteTarget::Storage { .. }
                        )
                    });
                    if !writes_through_alias {
                        return canonical;
                    }
                    match crate::flow::rebase_exact_local_place(
                        program,
                        state_symbol,
                        statement_index,
                        canonical.clone(),
                        build.call_frames,
                    ) {
                        Some(exact) => exact,
                        None => {
                            if let PlaceRoot::Symbol(root) = canonical.root
                                && let Some(candidates) = build.reference_candidate_places_at(
                                    program,
                                    state_symbol,
                                    statement_index,
                                    root,
                                )
                            {
                                candidate_targets = candidates
                                    .iter()
                                    .cloned()
                                    .map(|mut candidate| {
                                        candidate.segments.extend_from_slice(&canonical.segments);
                                        candidate
                                    })
                                    .collect();
                            }
                            canonical
                        }
                    }
                })
                .map(|canonical| {
                    crate::semantic::places::append_place_with_segments(
                        semantic,
                        canonical.root,
                        &canonical.segments,
                    )
                })?;
            let source_place = contextual_expression_place(
                program,
                semantic,
                machine_symbol,
                state_symbol,
                statement_index,
                assignment.value,
            );
            (target_place, assignment.value, source_place)
        }
        StatementNode::Call(_) | StatementNode::Expression(_) | StatementNode::Transition(_) => {
            return None;
        }
    };
    Some(StatementWrite {
        target_place,
        source_expression,
        source_place,
        candidate_targets,
    })
}

/// Narrow each runtime `Index` segment of `target_place` whose selector
/// value is pinned in the active contexts, and report whether the narrowed
/// place is stable enough to carry value facts.
pub(super) fn narrow_pinned_selectors(
    program: &symbol_resolved_trees_to_typed_trees::typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    state_symbol: SymbolHandle,
    statement_index: usize,
    context_handles: &[crate::fact_plan::FactContextHandle],
    mut target_place: PlaceHandle,
) -> (PlaceHandle, bool) {
    // A runtime index can change without writing the collection. Until value
    // facts retain index dependencies, only immutable selectors carry values.
    // A selector whose live value is already pinned names the one exact
    // element this write hits, though: narrow its `Index` segment to the
    // `FixedIndex` it selects at this point -- the same narrowing the
    // requires-side read performs (`projected_formal_leaf_value` in
    // checks/contracts/direct.rs) -- so the recorded fact lands on the place
    // callers actually read instead of an unreachable runtime spelling.
    let mut target_segments: Vec<crate::fact_plan::PlaceSegment> = semantic
        .place_segments
        .span_or_empty(semantic.places.get(target_place).segments)
        .to_vec();
    for segment in &mut target_segments {
        let crate::fact_plan::PlaceSegment::Index { expression } = *segment else {
            continue;
        };
        let Some(selector) = crate::flow::canonical_place_from_expression_in_state(
            program,
            state_symbol,
            statement_index,
            expression,
        ) else {
            continue;
        };
        let Some(crate::fact_plan::ScalarValue::Integer(index)) =
            crate::values::scalar_value_at_place(
                program,
                semantic,
                context_handles
                    .iter()
                    .map(|handle| semantic.contexts.get(*handle)),
                &selector,
            )
        else {
            continue;
        };
        if let Some(index) = index.to_u64().and_then(|index| usize::try_from(index).ok()) {
            *segment = crate::fact_plan::PlaceSegment::FixedIndex { index };
        }
    }
    let stable_value_target = target_segments.iter().all(|segment| {
        matches!(
            segment,
            crate::fact_plan::PlaceSegment::Field { .. }
                | crate::fact_plan::PlaceSegment::Case { .. }
                | crate::fact_plan::PlaceSegment::FixedIndex { .. }
        )
    });
    if stable_value_target
        && target_segments
            != semantic
                .place_segments
                .span_or_empty(semantic.places.get(target_place).segments)
    {
        let root = semantic.places.get(target_place).root;
        target_place =
            crate::semantic::places::append_place_with_segments(semantic, root, &target_segments);
    }
    (target_place, stable_value_target)
}
