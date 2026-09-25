//! Declared domains a write establishes: initializing or assigning a
//! domain-refined place records membership in each declared domain that
//! needs no provenance, and a write through a reference local keeps each
//! candidate's membership exactly when it held before the write.

use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn establish(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    build: &FlowBuildContext,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    statement: &StatementNode,
    target_place: PlaceHandle,
    candidate_targets: Vec<CanonicalPlace>,
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
                crate::lookup::machine_by_symbol(program, machine_symbol),
                crate::semantic::calls::find_state_in_machine(
                    program,
                    machine_symbol,
                    state_symbol,
                ),
            ) {
                (Some(machine), Some(state)) => {
                    crate::facts::field_domain::assignment_target_domain_identities(
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
            crate::facts::field_domain::domain_constraint_identities(program, local.type_reference)
        }
        _ => Vec::new(),
    };
    // Routed membership follows its checked source; an annotation cannot
    // establish provenance. Initializer and write checks consume the source
    // facts before this statement, never these newly seeded predicate facts.
    let declared_target_domains: Vec<_> = declared_target_domains
        .into_iter()
        .filter(|(symbol, _)| {
            !crate::facts::field_domain::domain_requires_provenance(program, *symbol)
        })
        .collect();
    for (domain_symbol, semantic_domain) in &declared_target_domains {
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
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
                domain_symbol: *domain_symbol,
                semantic_domain: *semantic_domain,
            },
        });
        semantic.append_ref(refs, fact);
    }
    for candidate in candidate_targets {
        let candidate_place = crate::semantic::places::append_place_with_segments(
            semantic,
            candidate.root,
            &candidate.segments,
        );
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
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(candidate_place),
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
                    domain_symbol: *domain_symbol,
                    semantic_domain: *semantic_domain,
                },
            });
            semantic.append_ref(refs, fact);
        }
    }
}
