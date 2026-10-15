//! Declared and authored facts handed back through captured call referents.
//!
//! A callee's normal return is a default-domain consumption point for every
//! mutable or write-only referent it received, `self` included
//! (checks/contracts/exits/result_domains.rs), so after the call each such
//! referent is known to satisfy the field facts the callee assumed on entry.
//! Call invalidation retires the actual's facts along the callee's write
//! frame; this phase re-seeds exactly those paths on the actual's storage
//! place. Pre-live concrete memberships also survive conservative overlap
//! when every affecting writer returns the same qualified occurrence. Writer
//! identity comes from its callee-relative formal, never selector spelling:
//! a guarantee about one captured index cannot excuse another argument's write.

use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
use crate::flow::reference_spans;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{BorrowCallFact, DomainFacts, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, PlaceSegment, ProgramPoint,
    QualificationEvidence,
};

/// Re-seed the declared field facts of every exclusive borrowed referent the
/// call hands back, on the referent's exact storage place.
pub(in crate::flow) fn append_call_referent_field_domain_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    pre_contexts: HandleSpan<FlowSemanticContextRef>,
    exit: &mut CallFlowContexts,
    argument_sources: Option<&super::ArgumentSources<'_, '_>>,
    domains: &DomainFacts,
    writes: Option<&[crate::flow::mutation::CallMutationWrite]>,
) {
    let Some(parameters) = super::super::calls::memoized_call_target_parameters(
        program,
        build,
        borrow_call.target_symbol,
    ) else {
        return;
    };
    let mut rows = Vec::new();
    let mut preserved = Vec::new();
    let mut guarantees = Vec::new();
    let parameter_claims = build
        .call_parameter_identities
        .get(&borrow_call.target_symbol)
        .cloned();
    for (argument_position, parameter) in parameters.iter().enumerate() {
        let referent =
            crate::checks::contracts::exclusive_referent_type(program, parameter.type_reference);
        // Every exclusive borrowed formal, including a receiver, restores its
        // declared fields. Receiver syntax does not change return obligations.
        let paths = build
            .referent_type_paths
            .entry(parameter.type_reference)
            .or_insert_with(|| {
                if !referent.is_valid() {
                    return None;
                }
                let paths = validation::field_domains::declared_result_field_domain_paths(
                    program, referent,
                );
                let mut paths = paths
                    .into_iter()
                    .filter(|(_, domain_symbol, _)| {
                        crate::checks::contracts::value_provable_domain(program, *domain_symbol)
                    })
                    .collect::<Vec<_>>();
                // Root and field handback share the same normal-return
                // obligation. The declaration is a callee obligation, not
                // permission to repair an unrelated caller occurrence.
                paths.extend(
                    validation::field_domains::domain_constraint_identities(
                        program,
                        parameter.type_reference,
                    )
                    .into_iter()
                    .map(|(domain, identity)| (Vec::new(), domain, identity)),
                );
                if paths.is_empty() {
                    None
                } else {
                    Some(std::rc::Rc::new(paths))
                }
            })
            .clone()
            .unwrap_or_default();
        let mut paths = paths.as_ref().clone();
        if argument_sources
            .is_some_and(|sources| sources.observation_survives(argument_position, &[]))
        {
            for (position, domain, identity, declared) in
                parameter_claims.iter().flat_map(|claims| claims.iter())
            {
                if *position == argument_position
                    && !declared
                    && validation::field_domains::domain_requires_provenance(program, *domain)
                {
                    let row = (Vec::new(), *domain, *identity);
                    if !paths.contains(&row) {
                        paths.push(row);
                    }
                }
            }
        }
        let Some(targets) =
            argument_sources.and_then(|sources| sources.referent_places(argument_position))
        else {
            continue;
        };
        // Facts live on the evaluated reference's captured backing. A
        // reference result may name one of finitely many candidates;
        // the callee guarantees the rows only on the candidate it received,
        // and the others were merely retired conservatively. Alternative
        // origins go through the shared all-writers preservation judgment.
        for storage in targets {
            for (path, domain_symbol, semantic_domain) in paths.iter() {
                if let Some(place) = crate::flow::project_captured_view_place(storage.clone(), path)
                {
                    guarantees.push((parameter.symbol, place, *domain_symbol, *semantic_domain));
                }
                if targets.len() > 1 {
                    // Alternative origins need the all-writers preservation
                    // judgment below, not independent unconditional facts.
                    continue;
                }
                rows.push((
                    storage.clone(),
                    path.clone(),
                    *domain_symbol,
                    *semantic_domain,
                ));
            }
        }
    }
    // Invalidation deliberately forgets which concrete element a runtime
    // selector denotes. Preserve only an already-live row, and only if every
    // overlapping contribution is restored by its own captured formal. Domain
    // dependency overlap uses the very same judgment as invalidation.
    if let Some(writes) = writes
        && !guarantees.is_empty()
    {
        for reference in build
            .contexts
            .semantic_context_refs
            .span_or_empty(pre_contexts)
        {
            for fact_reference in semantic
                .refs
                .span_or_empty(semantic.contexts.get(reference.context).facts)
            {
                let fact = semantic.facts.get(fact_reference.fact);
                let (
                    FactPlace::Place(handle),
                    FactPayload::DomainMembership {
                        domain_symbol,
                        semantic_domain,
                        ..
                    }
                    | FactPayload::ContractDomainMembership {
                        domain_symbol,
                        semantic_domain,
                        ..
                    },
                ) = (fact.place, fact.payload)
                else {
                    continue;
                };
                if !semantic_domain.is_valid() {
                    continue;
                }
                let Some(place) = crate::flow::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic.places.get(handle),
                ) else {
                    continue;
                };
                if !place
                    .segments
                    .iter()
                    .all(crate::flow::place_segment_has_static_identity)
                {
                    continue;
                }
                let mut affected = false;
                let restored = writes.iter().all(|write| {
                    if crate::flow::domain::matching_mutation_for_fact_place(
                        program,
                        semantic,
                        domains,
                        fact,
                        handle,
                        std::slice::from_ref(&write.place),
                    )
                    .is_none()
                    {
                        return true;
                    }
                    affected = true;
                    let Some(source) = &write.source else {
                        return false;
                    };
                    guarantees
                        .iter()
                        .any(|(parameter, guarantee, domain, identity)| {
                            source.root.as_symbol() == Some(*parameter)
                                && write_stays_in_captured_storage(program, parameters, source)
                                && *domain == domain_symbol
                                && *identity == semantic_domain
                                && same_occurrence_when_selected(program, &place, guarantee)
                        })
                });
                if affected && restored {
                    // This is a frame judgment about a pre-existing fact,
                    // not a new claim issued on this exact path by the call.
                    // Keep its original issuer and proof point for the
                    // qualification join; only its liveness changes here.
                    let row = (place, fact_reference.fact);
                    if !preserved.contains(&row) {
                        preserved.push(row);
                    }
                }
            }
        }
    }
    if rows.is_empty() && preserved.is_empty() {
        return;
    }
    let point = ProgramPoint::CallEnsures {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
    };
    let evidence = QualificationEvidence::from_origin(
        language_semantics::QualificationEvidenceOrigin::Propagated,
        borrow_call.target_symbol,
    );
    // One context per exact place, so a later write to one field retires
    // only that field's row (flow/transfers keeps the same granularity).
    let mut contexts: Vec<(validation::CanonicalPlace, HandleSpan<facts::FactRef>)> = Vec::new();
    for (place, fact) in preserved {
        if let Some((_, refs)) = contexts
            .iter_mut()
            .find(|(candidate, _)| *candidate == place)
        {
            semantic.append_ref(refs, fact);
        } else {
            let mut refs = HandleSpan::empty();
            semantic.append_ref(&mut refs, fact);
            contexts.push((place, refs));
        }
    }
    for (storage, path, domain_symbol, semantic_domain) in rows {
        let Some(place) = crate::flow::project_captured_view_place(storage, &path) else {
            continue;
        };
        let place_handle =
            crate::semantic::places::append_place(semantic, place.root, &place.segments);
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place_handle),
            point,
            origin: FactOrigin::CallEnsures,
            evidence,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol,
                semantic_domain,
            },
        });
        if let Some((_, refs)) = contexts
            .iter_mut()
            .find(|(candidate, _)| *candidate == place)
        {
            semantic.append_ref(refs, fact);
        } else {
            let mut refs = HandleSpan::empty();
            semantic.append_ref(&mut refs, fact);
            contexts.push((place, refs));
        }
    }
    for (_, refs) in contexts {
        reference_spans::append_semantic_context(semantic, build, exit, point, refs);
    }
}

/// A callee-relative inline write stays within this captured occurrence.
/// A stored reference crosses to another home, so its formal root alone is
/// insufficient provenance for restoring the containing owner's promises.
fn write_stays_in_captured_storage(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    source: &validation::CanonicalPlace,
) -> bool {
    let Some(parameter) = parameters
        .iter()
        .find(|parameter| source.root.as_symbol() == Some(parameter.symbol))
    else {
        return false;
    };
    let Some(shell) = validation::reference_shell(program, parameter.type_reference) else {
        return false;
    };
    let mut reference = shell.referee;
    for segment in &source.segments {
        if validation::reference_shell(program, reference).is_some() {
            return false;
        }
        let Some(projected) = validation::project_type_reference_from_segments(
            program,
            reference,
            std::slice::from_ref(segment),
        ) else {
            return false;
        };
        reference = projected;
    }
    // A write summary ending at a stored pointer can denote its referent,
    // not only replacement of the descriptor slot. Without that distinction
    // in the summary, it cannot establish confinement to the inline owner.
    validation::reference_shell(program, reference).is_none()
}

/// Conditional identity of a concrete row and a captured selected occurrence.
/// An unknown single-element coordinate may select that concrete index; it
/// does not equate two separately evaluated selectors or a whole range.
fn same_occurrence_when_selected(
    program: &dyn ::validation::CheckingInput,
    concrete: &validation::CanonicalPlace,
    selected: &validation::CanonicalPlace,
) -> bool {
    concrete.root == selected.root
        && concrete.segments.len() == selected.segments.len()
        && concrete
            .segments
            .iter()
            .zip(&selected.segments)
            .all(|(left, right)| match (*left, *right) {
                (PlaceSegment::FixedIndex { .. }, PlaceSegment::Index { expression }) => {
                    expression.is_valid()
                        && program.expression_table.range_node(expression).is_none()
                }
                _ => crate::flow::static_place_segment_pair_equal(*left, *right)
                    .unwrap_or(*left == *right),
            })
}
