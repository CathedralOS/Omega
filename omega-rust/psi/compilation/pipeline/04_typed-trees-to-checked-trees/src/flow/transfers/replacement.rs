//! A selected owned replacement can preserve a collection's field contracts.
//!
//! Conservative overlap retires every possible selected element's membership.
//! Each exact old leaf nevertheless keeps its qualification if its unchanged
//! value had that qualification and the completed replacement independently
//! satisfies the same conditional leaf contract. This reactivates an existing
//! fact; it does not distribute the replacement's value, tag, issuer or equality
//! among the candidates. Previously inactive leaves have no membership to
//! reactivate and are not manufactured from their declarations here.
use super::write_target::StatementWrite;
use crate::flow::{CaseEvidence, FlowBuildContext, MembershipEvidence};
use arena::{Handle, HandleSpan};
use checked_trees::{FlowSemanticContextRef, FlowStatementFact};
use facts::{FactPlan, FactRef, PlaceSegment, ProgramPoint};

pub(super) fn preserve_owned_memberships(
    program: &dyn validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &FlowBuildContext,
    write: &StatementWrite,
    statement: Handle<FlowStatementFact>,
    before_store: HandleSpan<FlowSemanticContextRef>,
    point: ProgramPoint,
    references: &mut HandleSpan<FactRef>,
) {
    // Multiple captured referents need an all-alternatives frame judgment.
    // This owner handles one exact captured address, even when its selector
    // leaves which concrete element it denotes unknown.
    if !write.target_is_exact {
        return;
    }
    let ProgramPoint::Statement {
        machine_symbol,
        state_symbol,
        statement_index,
    } = point
    else {
        return;
    };
    let Some((machine, state)) =
        validation::machine_state_by_symbols(program, machine_symbol, state_symbol)
    else {
        return;
    };
    // An authored indexing operator may return an unrelated owned value.
    // Syntactic projection alone cannot lend the collection's qualifications
    // to that result; use storage evidence only for selected builtin geometry.
    if !validation::value_has_builtin_coordinates(program, machine, state, write.source_expression)
    {
        return;
    }
    let target = semantic.places.get(write.target_place);
    let target_segments = semantic.place_segments.span_or_empty(target.segments);
    if !target_segments.iter().any(|segment| matches!(segment, PlaceSegment::Index { .. }))
        || !target_segments.iter().all(|segment| {
            crate::flow::place_segment_has_static_identity(segment)
                || matches!(segment, PlaceSegment::Index { expression }
                    if expression.is_valid() && program.expression_table.range_node(*expression).is_none())
        })
    {
        return;
    }
    let stored = build.control.statements.get(statement).stored_value;
    if !build.control.operands.is_valid(stored) {
        return;
    }
    let replacement = build.control.operands.get(stored);
    if replacement.expression != write.source_expression
        || !validation::has_contents(
            program,
            replacement.type_reference,
            &[],
            validation::ContentsRequirement::BorrowFree,
        )
    {
        return;
    }
    let Some(source) = write.source_place.and_then(|source| {
        crate::flow::canonical_place_from_semantic_place(
            program,
            semantic,
            semantic.places.get(source),
        )
    }) else {
        return;
    };
    let completed = crate::flow::operand_contexts(
        program,
        &build.contexts,
        &build.control,
        semantic,
        replacement,
        replacement.type_reference,
        replacement.constraints,
    );
    let memberships = MembershipEvidence::prepare(program, semantic, &completed);
    let Some(cases) = CaseEvidence::prepare(
        program,
        semantic,
        &completed,
        machine_symbol,
        state_symbol,
        statement_index,
    ) else {
        return;
    };
    let contexts = build
        .contexts
        .semantic_context_refs
        .span_or_empty(before_store)
        .iter()
        .map(|reference| reference.context);
    let mut preserved = Vec::new();
    for handle in semantic.unique_context_facts(contexts) {
        let fact = semantic.facts.get(handle);
        let Some((_, _, domain, identity)) = fact.payload.domain_membership() else {
            continue;
        };
        let Some(place) = fact.place.as_place() else {
            continue;
        };
        let candidate = semantic.places.get(place);
        let segments = semantic.place_segments.span_or_empty(candidate.segments);
        if !identity.is_valid()
            || candidate.root != target.root
            || segments.len() <= target_segments.len()
            || !segments
                .iter()
                .all(crate::flow::place_segment_has_static_identity)
            || !target_segments
                .iter()
                .zip(segments)
                .all(|(target, candidate)| {
                    matches!(
                        (target, candidate),
                        (PlaceSegment::Index { .. }, PlaceSegment::FixedIndex { .. })
                    ) || crate::flow::static_place_segment_pair_equal(*target, *candidate)
                        == Some(true)
                })
        {
            continue;
        }
        let relative = &segments[target_segments.len()..];
        let mut projected = source.clone();
        projected.segments.extend_from_slice(relative);
        if cases.path_is_inactive(&projected)
            || crate::flow::literal_value_path_is_inactive(
                program,
                replacement.expression,
                replacement.type_reference,
                relative,
            )
            || memberships.contains(&projected, domain, identity)
        {
            // Keep the old fact's original proof point and evidence, as call
            // preservation does. No new source correspondence is asserted.
            preserved.push(handle);
        }
    }
    drop(memberships);
    for fact in preserved {
        semantic.append_ref(references, fact);
    }
}
