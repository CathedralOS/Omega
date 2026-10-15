//! Declared domains a write establishes: initializing or assigning a
//! domain-refined place records membership in each declared domain that
//! needs no provenance, and a write through a reference local keeps each
//! candidate's membership exactly when it held before the write.

use validation::CanonicalPlace;

use super::{
    ExpressionHandle, Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FlowBuildContext,
    FlowSemanticContextRef, HandleSpan, PlaceHandle, ProgramPoint, QualificationEvidence,
    StatementNode, SymbolHandle,
};

#[allow(clippy::too_many_arguments)]
pub(super) fn establish(
    program: &dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    target_place: PlaceHandle,
    target_is_exact: bool,
    mut candidate_targets: Vec<CanonicalPlace>,
    assignment_source_contexts: HandleSpan<FlowSemanticContextRef>,
    refs: &mut HandleSpan<facts::FactRef>,
) {
    // #66 read-narrowing across a write: initializing or assigning any
    // domain-refined declared place ESTABLISHES that destination's domain. The
    // write checker separately proves the source satisfies the declaration;
    // recording the fact here makes a checked LET usable at later call and
    // operator boundaries just like a checked reassignment. An uninitialized
    // local grants nothing.
    //
    // The fact carries the identity the typer interned on the declared
    // constraint, not only the definition symbol: an indexed application such
    // as `Extent in Granted & Resident<P, T>` is proved at a call `requires`
    // only against an exact instance identity, so a restated local that
    // recorded `NULL` here kept the domain for weakening and linearity but was
    // unprovable as a call premise.
    let declared_target_domains = match statement {
        StatementNode::Assignment(assignment) => {
            match (
                program.machine_by_symbol(machine_symbol),
                validation::find_state_in_machine(program, machine_symbol, state_symbol),
            ) {
                (Some(machine), Some(state)) => {
                    validation::field_domains::assignment_target_domain_identities(
                        program,
                        machine,
                        state,
                        assignment.target,
                    )
                }
                _ => Vec::new(),
            }
        }
        StatementNode::LocalData(local) if local.initial_value.is_valid() => {
            validation::field_domains::domain_constraint_identities(program, local.type_reference)
        }
        _ => Vec::new(),
    };
    // Routed membership follows its checked source; an annotation cannot
    // establish provenance. Initializer and write checks consume the source
    // facts before this statement, never these newly seeded predicate facts.
    let declared_target_domains: Vec<_> = declared_target_domains
        .into_iter()
        .filter(|(symbol, _)| {
            !validation::field_domains::domain_requires_provenance(program, *symbol)
        })
        .collect();
    let membership_fact =
        |place: PlaceHandle,
         domain_symbol: SymbolHandle,
         semantic_domain: language_semantics::SemanticDomainId| Fact {
            place: FactPlace::Place(place),
            point: ProgramPoint::Statement {
                machine_symbol,
                state_symbol,
                statement_index,
            },
            origin: FactOrigin::StatementTransfer,
            evidence: QualificationEvidence::from_origin(
                language_semantics::QualificationEvidenceOrigin::CheckedValidation,
                state_symbol,
            ),
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol,
                semantic_domain,
            },
        };
    for (domain_symbol, semantic_domain) in
        declared_target_domains.iter().filter(|_| target_is_exact)
    {
        let fact = semantic.append_fact(membership_fact(
            target_place,
            *domain_symbol,
            *semantic_domain,
        ));
        semantic.append_ref(refs, fact);
    }
    let target = semantic.places.get(target_place);
    let target_segments = semantic.place_segments.span_or_empty(target.segments);
    if target_is_exact
        && target_segments.iter().any(|segment| {
            matches!(
                segment,
                facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedIndex { .. }
            )
        })
    {
        // The write checker establishes the destination's declared predicate
        // on the selected element. Its uniform collection siblings keep their
        // old predicate, even when a context containing several entry facts is
        // retired as a whole. Restore only static leaves live before this
        // replacement, never all cells inferred from an array type. Value
        // equality is not preserved or copied to unselected siblings.
        for reference in build
            .contexts
            .semantic_context_refs
            .span_or_empty(assignment_source_contexts)
        {
            let context = semantic.contexts.get(reference.context);
            for reference in semantic.refs.span_or_empty(context.facts) {
                let fact = semantic.facts.get(reference.fact);
                let Some((_, _, symbol, domain)) = fact.payload.domain_membership() else {
                    continue;
                };
                if !declared_target_domains.contains(&(symbol, domain)) {
                    continue;
                }
                let Some(place) = fact.place.as_place() else {
                    continue;
                };
                let candidate = semantic.places.get(place);
                let segments = semantic.place_segments.span_or_empty(candidate.segments);
                if candidate.root != target.root
                    || segments.len() != target_segments.len()
                    || !segments.iter().all(|segment| {
                        matches!(
                            segment,
                            facts::PlaceSegment::Field { .. }
                                | facts::PlaceSegment::Case { .. }
                                | facts::PlaceSegment::FixedIndex { .. }
                        )
                    })
                    || !target_segments
                        .iter()
                        .zip(segments)
                        .all(|(target, candidate)| {
                            matches!(
                                (target, candidate),
                                (
                                    facts::PlaceSegment::Index { .. }
                                        | facts::PlaceSegment::FixedIndex { .. },
                                    facts::PlaceSegment::FixedIndex { .. }
                                )
                            ) || target == candidate
                        })
                {
                    continue;
                }
                let candidate = CanonicalPlace {
                    root: candidate.root,
                    segments: segments.to_vec(),
                };
                if !candidate_targets.contains(&candidate) {
                    candidate_targets.push(candidate);
                }
            }
        }
    }
    for candidate in candidate_targets {
        let candidate_place =
            crate::semantic::places::append_place(semantic, candidate.root, &candidate.segments);
        for (domain_symbol, semantic_domain) in &declared_target_domains {
            let was_live = build
                .contexts
                .semantic_context_refs
                .span_or_empty(assignment_source_contexts)
                .iter()
                .any(|reference| {
                    semantic
                        .context_view(semantic.contexts.get(reference.context))
                        .proves_place_domain_membership_in_program(
                            program,
                            candidate_place,
                            *domain_symbol,
                        )
                });
            if !was_live {
                continue;
            }
            let fact = semantic.append_fact(membership_fact(
                candidate_place,
                *domain_symbol,
                *semantic_domain,
            ));
            semantic.append_ref(refs, fact);
        }
    }
}
