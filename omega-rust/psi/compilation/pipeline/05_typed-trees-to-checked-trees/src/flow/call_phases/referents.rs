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

use super::super::calls::memoized_call_target_parameters;
use super::super::calls::memoized_find_call_site;
use super::invalidation::ObservedCallMutation;
use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
mod batches;
mod guarantees;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{BorrowCallFact, DomainFacts, FlowSemanticContextRef};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan};

/// Re-seed the declared field facts of every exclusive borrowed referent the
/// call hands back, on the referent's exact storage place.
pub(in crate::flow) fn append_call_referent_field_domain_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &checked_trees::BorrowFacts,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    pre_contexts: HandleSpan<FlowSemanticContextRef>,
    exit: &mut CallFlowContexts,
    argument_sources: Option<&super::ArgumentSources<'_, '_>>,
    domains: &DomainFacts,
    writes: Option<&[ObservedCallMutation]>,
) {
    let trace = std::env::var("OMEGA_STATE_REFERENCE_DIAGNOSTICS")
        .is_ok_and(|selected| selected == "1" || selected == machine.name.as_str());
    let key = (
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let selected =
        memoized_find_call_site(program, build, key.0, key.1, key.2, key.3).and_then(|site| {
            match site {
                validation::CallSite::Expression { call, .. } => {
                    program.requirement_call_signature(machine.symbol, call)
                }
                _ => None,
            }
        });
    let Some(parameters) = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| memoized_call_target_parameters(program, build, borrow_call.target_symbol))
    else {
        if trace {
            eprintln!(
                "returned-field-contracts target={:?} parameters=unavailable",
                borrow_call.target_symbol
            );
        }
        return;
    };
    let mut rows = Vec::new();
    let mut preserved = Vec::new();
    let mut guarantees = Vec::new();
    let param_key = (
        borrow_call.target_symbol,
        selected.map_or(0, |s| std::ptr::from_ref(s) as usize),
    );
    let parameter_claims = build.call_parameter_identities.get(&param_key).cloned();
    let body_is_present = validation::find_state(program, borrow_call.target_symbol)
        .is_some_and(|(machine, _)| machine.body_is_present);
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
                // Normal-return checking proves these exact field obligations
                // from live evidence, including routed memberships. A checked
                // replacement may carry a different owned value; the caller
                // receives its restored field promise, not a resurrection of
                // the value removed on entry. Captured referents below bind
                // that promise to the actual storage passed to this call.
                // Additional root qualifications are input preconditions,
                // not normal-return promises. Reintroducing them here would
                // resurrect routed membership after an unissued replacement
                // and conceal corruption of a caller's qualified field.
                // Explicit guarantees and unchanged observations are joined
                // independently below and in call-parameter contract facts.
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
            // An explicit postcondition restores the captured referent whether
            // the domain needs provenance or only a predicate. Include it in
            // the same all-writers judgment as declared child obligations;
            // an entry qualification alone still supplies no return promise.
            for (position, domain, identity) in
                parameter_claims.iter().flat_map(|claims| claims.iter())
            {
                if *position == argument_position {
                    let row = (Vec::new(), *domain, *identity);
                    crate::shared::collections::push_unique(&mut paths, row);
                }
            }
        }
        let targets =
            argument_sources.and_then(|sources| sources.referent_places(argument_position));
        if trace {
            eprintln!(
                "returned-field-contracts target={:?} argument={argument_position} parameter={:?} declared={paths:?} captured={targets:?}",
                borrow_call.target_symbol, parameter.symbol
            );
        }
        let Some(targets) = targets else {
            continue;
        };
        // Facts live on the evaluated reference's captured backing. A
        // reference result may name one of finitely many candidates;
        // the callee guarantees the rows only on the candidate it received,
        // and the others were merely retired conservatively. Alternative
        // origins go through the shared all-writers preservation judgment.
        for storage in targets {
            // The referent's own declared constraints join only the returned
            // guarantee vocabulary. A checked write through this formal's
            // typed view is the one route by which the callee could have
            // touched the captured storage. Restoration additionally rejoins
            // the performed assignment's checked destination presentation:
            // an unqualified recast retains storage coordinates but supplies
            // no evidence for this constraint. A deeper write cannot restore
            // a qualification on the whole referent by address confinement.
            for (domain_symbol, semantic_domain) in
                validation::field_domains::domain_constraint_identities(
                    program,
                    parameter.type_reference,
                )
            {
                guarantees.push(guarantees::ReturnedGuarantee {
                    parameter: parameter.symbol,
                    place: storage.clone(),
                    domain: domain_symbol,
                    identity: semantic_domain,
                    proven_at_exit: false,
                });
            }
            for (path, domain_symbol, semantic_domain) in paths.iter() {
                let authored_guarantee = path.is_empty()
                    && parameter_claims
                        .iter()
                        .flat_map(|claims| claims.iter())
                        .any(|(position, domain, identity)| {
                            *position == argument_position
                                && *domain == *domain_symbol
                                && *identity == *semantic_domain
                        });
                // An additional input-domain constraint is not a boundary's
                // output promise. Authored guarantees must still be published
                // provisionally: the call-result authority check validates
                // them, and omitting their row would omit that obligation.
                if !body_is_present
                    && path.is_empty()
                    && !authored_guarantee
                    && !crate::checks::contracts::value_provable_domain(program, *domain_symbol)
                {
                    continue;
                }
                let Some(place) = crate::flow::project_captured_view_place(storage.clone(), path)
                else {
                    continue;
                };
                // A boundary's declared field contract restores an existing
                // input qualification, but cannot mint routed output authority.
                // Require that exact applied domain on this captured actual
                // before invocation. A neighbor, another index, or a merely
                // equal carrier supplies no provenance for this handback.
                if !body_is_present
                    && !authored_guarantee
                    && !crate::checks::contracts::value_provable_domain(program, *domain_symbol)
                    && !build
                        .contexts
                        .semantic_context_refs
                        .span_or_empty(pre_contexts)
                        .iter()
                        .any(|reference| {
                            semantic
                                .context_view(semantic.contexts.get(reference.context))
                                .facts()
                                .any(|fact| {
                                    let Some((_, _, domain, identity)) =
                                        fact.payload.domain_membership()
                                    else {
                                        return false;
                                    };
                                    if domain != *domain_symbol || identity != *semantic_domain {
                                        return false;
                                    }
                                    fact.place
                                        .as_place()
                                        .and_then(|source| {
                                            crate::flow::canonical_place_from_semantic_place(
                                                program,
                                                semantic,
                                                semantic.places.get(source),
                                            )
                                        })
                                        .as_ref()
                                        == Some(&place)
                                })
                        })
                {
                    continue;
                }
                guarantees.push(guarantees::ReturnedGuarantee {
                    parameter: parameter.symbol,
                    place,
                    domain: *domain_symbol,
                    identity: *semantic_domain,
                    proven_at_exit: true,
                });
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
    // The consumed holder returns its exact captured source. Pure declared
    // predicates follow the normal typed handback; routed descendants also
    // require every historical input supply's exact membership. The same
    // immutable receipt is independently replayed by content admission.
    for returned in crate::flow::retained_borrows::handbacks(
        program,
        borrow,
        &build.control,
        machine,
        state,
        borrow_call,
        argument_sources,
    ) {
        for membership in crate::flow::retained_borrows::memberships(
            program,
            semantic,
            &build.contexts,
            &build.control,
            &returned,
        ) {
            rows.push((
                membership.place,
                Vec::new(),
                membership.domain,
                membership.instance,
            ));
        }
    }
    // Invalidation deliberately forgets which concrete element a runtime
    // selector denotes. Preserve only an already-live row, and only if every
    // overlapping contribution is restored by its own captured formal. Domain
    // dependency overlap uses the very same judgment as invalidation.
    if let Some(writes) = writes.filter(|writes| !writes.is_empty()) {
        // Confinement depends only on this immutable write path and its
        // selected formal, not on a live fact or a candidate guarantee. A
        // qualified fixed array can contribute thousands of each: proving
        // the same reference/field/index geometry inside that join repeated
        // millions of type walks per call. Prepare only this static premise;
        // overlap and the all-writers restoration verdict stay pass-local.
        let confined_writes: Vec<bool> = writes
            .iter()
            .map(|contribution| {
                let Some(crate::flow::mutation::StorageRegion::Declared(source)) =
                    &contribution.write.source
                else {
                    return false;
                };
                matches!(
                    contribution.write.region,
                    crate::flow::mutation::StorageRegion::Declared(_)
                ) && super::copied_guarantees::confined_write_parameter(program, parameters, source)
                    .is_some()
            })
            .collect();
        let captured_writes: Vec<_> = writes
            .iter()
            .zip(&confined_writes)
            .map(|(write, confined)| {
                if !*confined {
                    return None;
                }
                let crate::flow::mutation::StorageRegion::Declared(source) =
                    write.write.source.as_ref()?
                else {
                    return None;
                };
                argument_sources?.inline_write_places(source)
            })
            .collect();
        let callee = validation::find_state(program, borrow_call.target_symbol);
        let returned_classes: Vec<_> = writes
            .iter()
            .zip(&confined_writes)
            .map(|(contribution, confined)| {
                if !*confined {
                    return None;
                }
                let crate::flow::mutation::StorageRegion::Declared(source) =
                    contribution.write.source.as_ref()?
                else {
                    return None;
                };
                let (callee, entry) = callee?;
                let root = source.root.as_symbol()?;
                let formal = program.state_parameter_by_symbol(entry, root)?;
                if !parameters.iter().any(|selected| {
                    selected.symbol == formal.symbol
                        && selected.type_reference == formal.type_reference
                }) {
                    // A different applied telescope needs its own completed
                    // body evidence; source symbol agreement alone is weaker.
                    return None;
                }
                let place = validation::CanonicalPlace {
                    root: facts::PlaceRoot::Symbol(root),
                    segments: Vec::new(),
                };
                let origins = crate::flow::entry_origins::project_invocation_place(
                    program, build, callee, entry, 0, &place, true,
                )?;
                match origins.as_slice() {
                    [origin] if origin.segments.is_empty() => Some(
                        validation::field_domains::ByteSequencePredicate::ALL.map(|predicate| {
                            build.normal_return_predicates.proves(
                                program,
                                callee,
                                origin.root,
                                predicate,
                            )
                        }),
                    ),
                    _ => None,
                }
            })
            .collect();
        if confined_writes.iter().any(|&confined| confined) {
            let guarantees = guarantees::PreparedGuarantees::prepare(guarantees);
            let contexts = build
                .contexts
                .semantic_context_refs
                .span_or_empty(pre_contexts)
                .iter()
                .map(|reference| reference.context);
            for fact_handle in semantic.unique_context_facts(contexts) {
                let fact = semantic.facts.get(fact_handle);
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
                let required_byte_class =
                    validation::field_domains::domain_byte_predicate(program, domain_symbol)
                        .filter(|restriction| restriction.is_subslice_preserving());
                let semantic_place = semantic.places.get(handle);
                if !semantic
                    .place_segments
                    .span_or_empty(semantic_place.segments)
                    .iter()
                    .all(crate::flow::place_segment_has_static_identity)
                {
                    continue;
                }
                if !matches!(semantic_place.root, facts::PlaceRoot::Expression(_))
                    && !writes.iter().any(|contribution| {
                        contribution
                            .observations
                            .iter()
                            .any(|obs| obs.root == semantic_place.root)
                    })
                {
                    continue;
                }
                let Some(place) = crate::flow::canonical_place_from_semantic_place(
                    program,
                    semantic,
                    semantic_place,
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
                let mut prepared_writes = writes
                    .iter()
                    .zip(&confined_writes)
                    .zip(&captured_writes)
                    .zip(&returned_classes);
                let restored = prepared_writes.all(
                    |(((contribution, confined), captured), returned_classes)| {
                        if contribution
                            .write
                            .region
                            .preserves_owner_qualification(program, semantic, fact, handle)
                        {
                            return true;
                        }
                        let Some((mutated, _)) =
                            crate::flow::domain::matching_mutation_for_fact_place(
                                program,
                                semantic,
                                domains,
                                fact,
                                handle,
                                &contribution.observations,
                            )
                        else {
                            return true;
                        };
                        affected = true;
                        let write = &contribution.write;
                        let Some(crate::flow::mutation::StorageRegion::Declared(source)) =
                            &write.source
                        else {
                            return false;
                        };
                        if !*confined {
                            // A content guarantee does not restore an owner's
                            // descriptor fact merely because the loan is anchored
                            // there. Its own observation frontier must be proved.
                            return false;
                        }
                        // Invalidation may keep an unknown source selector even
                        // after evaluation captured one exact element. A disjoint
                        // captured write leaves this fact unchanged. Other alias
                        // namespaces keep the original conservative judgment.
                        let mutated = if let Some(captured) = captured
                            .as_ref()
                            .filter(|places| places.iter().all(|place| place.root == mutated.root))
                        {
                            match crate::flow::domain::matching_mutation_for_fact_place(
                                program, semantic, domains, fact, handle, captured,
                            ) {
                                Some((mutated, _)) => mutated,
                                None => return true,
                            }
                        } else {
                            mutated
                        };
                        // A per-byte class is a property of every element. An indexed
                        // store does not retire it when the callee's normal exits
                        // still prove that class on the formal that was written.
                        if required_byte_class.is_some_and(|required| {
                            completed_return_keeps_byte_class(
                                returned_classes.as_ref(),
                                required,
                                &place,
                                mutated,
                            )
                        }) {
                            return true;
                        }
                        guarantees.restores(
                            program,
                            source.root.as_symbol(),
                            &place,
                            domain_symbol,
                            semantic_domain,
                            match &write.region {
                                crate::flow::mutation::StorageRegion::Declared(region) => {
                                    Some(region)
                                }
                                _ => None,
                            },
                            write.checked_write_type,
                        )
                    },
                );
                if affected && restored {
                    // This is a frame judgment about a pre-existing fact,
                    // not a new claim issued on this exact path by the call.
                    // Keep its original issuer and proof point for the
                    // qualification join; only its liveness changes here.
                    preserved.push((place, fact_handle));
                }
            }
        }
    }
    if rows.is_empty() && preserved.is_empty() {
        return;
    }
    let point = crate::flow::call_ensures_point(machine, state, borrow_call);
    let evidence = crate::published::facts::propagated_evidence(borrow_call.target_symbol);
    // One context per exact place, so a later write to one field retires
    // only that field's row (flow/transfers keeps the same granularity).
    let mut contexts = preserved;
    if trace {
        eprintln!(
            "returned-field-contracts target={:?} republished={rows:?}",
            borrow_call.target_symbol
        );
    }
    for (storage, path, domain_symbol, semantic_domain) in rows {
        let Some(place) = crate::flow::project_captured_view_place(storage, &path) else {
            continue;
        };
        let place_handle =
            crate::published::semantic::places::append_place(semantic, place.root, &place.segments);
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
        contexts.push((place, fact));
    }
    batches::append_place_contexts(semantic, build, exit, point, contexts);
}

/// An indexed write preserves an existing per-byte membership only when every
/// completed normal return proves that class on this writer's exact referent.
/// The caller still supplies the original membership and all-writer join;
/// this body summary neither mints a domain instance nor grants storage access.
fn completed_return_keeps_byte_class(
    classes: Option<&[bool; validation::field_domains::ByteSequencePredicate::ALL.len()]>,
    required: validation::field_domains::ByteSequencePredicate,
    fact_place: &validation::CanonicalPlace,
    mutated: &validation::CanonicalPlace,
) -> bool {
    mutated.root == fact_place.root
        && mutated.segments.len() > fact_place.segments.len()
        && mutated.segments.starts_with(&fact_place.segments)
        && mutated.segments[fact_place.segments.len()..]
            .iter()
            .all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
        && classes.is_some_and(|classes| {
            validation::field_domains::ByteSequencePredicate::ALL
                .into_iter()
                .zip(classes)
                .any(|(predicate, proved)| predicate == required && *proved)
        })
}
