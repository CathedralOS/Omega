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
mod batches;
mod guarantees;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{BorrowCallFact, DomainFacts, FlowSemanticContextRef};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;

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
    writes: Option<&[super::invalidation::ObservedCallMutation]>,
) {
    let key = (
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let selected =
        super::super::calls::memoized_find_call_site(program, build, key.0, key.1, key.2, key.3)
            .and_then(|site| match site {
                validation::CallSite::Expression { call, .. } => {
                    program.requirement_call_signature(machine.symbol, call)
                }
                _ => None,
            });
    let Some(parameters) = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| {
            super::super::calls::memoized_call_target_parameters(
                program,
                build,
                borrow_call.target_symbol,
            )
        })
    else {
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
        let body_is_present = validation::find_state(program, borrow_call.target_symbol)
            .is_some_and(|(machine, _)| machine.body_is_present);
        if argument_sources
            .is_some_and(|sources| sources.observation_survives(argument_position, &[]))
        {
            for (position, domain, identity) in
                parameter_claims.iter().flat_map(|claims| claims.iter())
            {
                if *position == argument_position
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
    // Invalidation deliberately forgets which concrete element a runtime
    // selector denotes. Preserve only an already-live row, and only if every
    // overlapping contribution is restored by its own captured formal. Domain
    // dependency overlap uses the very same judgment as invalidation.
    if let Some(writes) = writes.filter(|writes| !writes.is_empty()) {
        let guarantees = guarantees::PreparedGuarantees::prepare(guarantees);
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
                ) && write_stays_in_captured_storage(program, parameters, source)
            })
            .collect();
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
            let Some(place) =
                crate::flow::canonical_place_from_semantic_place(program, semantic, semantic_place)
            else {
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
            let mut prepared_writes = writes.iter().zip(&confined_writes);
            let restored = prepared_writes.all(|(contribution, confined)| {
                if contribution
                    .write
                    .region
                    .preserves_owner_qualification(program, semantic, fact, handle)
                {
                    return true;
                }
                let Some((mutated, _)) = crate::flow::domain::matching_mutation_for_fact_place(
                    program,
                    semantic,
                    domains,
                    fact,
                    handle,
                    &contribution.observations,
                ) else {
                    return true;
                };
                affected = true;
                let write = &contribution.write;
                let Some(crate::flow::mutation::StorageRegion::Declared(source)) = &write.source
                else {
                    return false;
                };
                if !*confined {
                    // A content guarantee does not restore an owner's
                    // descriptor fact merely because the loan is anchored
                    // there. Its own observation frontier must be proved.
                    return false;
                }
                // A per-byte class is a property of every element. An indexed
                // store does not retire it when the callee's normal exits
                // still prove that class on the formal that was written.
                if element_store_keeps_byte_class(
                    program,
                    semantic,
                    build,
                    borrow_call.target_symbol,
                    source.root.as_symbol(),
                    &place,
                    mutated,
                    domain_symbol,
                ) {
                    return true;
                }
                guarantees.restores(
                    program,
                    source.root.as_symbol(),
                    &place,
                    domain_symbol,
                    semantic_domain,
                )
            });
            if affected && restored {
                // This is a frame judgment about a pre-existing fact,
                // not a new claim issued on this exact path by the call.
                // Keep its original issuer and proof point for the
                // qualification join; only its liveness changes here.
                preserved.push((place, fact_handle));
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
    let mut contexts = preserved;
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
        contexts.push((place, fact));
    }
    batches::append_place_contexts(semantic, build, exit, point, contexts);
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

/// An indexed store keeps a per-byte class when every state that writes an
/// element of the same formal still proves that class after the store.
/// The writing state names its own parameter; transitions carry that storage
/// into later states under a new symbol. Whole-carrier replacement, and a
/// store whose following frontier does not prove the class, stay retired.
fn element_store_keeps_byte_class(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &FlowBuildContext<'_>,
    target_symbol: SymbolHandle,
    parameter: Option<SymbolHandle>,
    fact_place: &validation::CanonicalPlace,
    mutated: &validation::CanonicalPlace,
    domain_symbol: SymbolHandle,
) -> bool {
    let Some(parameter) = parameter else {
        return false;
    };
    let Some(required) = validation::field_domains::domain_byte_predicate(program, domain_symbol)
    else {
        return false;
    };
    if !required.is_subslice_preserving()
        || mutated.root != fact_place.root
        || mutated.segments.len() <= fact_place.segments.len()
        || !mutated.segments.starts_with(&fact_place.segments)
        || !mutated.segments[fact_place.segments.len()..]
            .iter()
            .all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Index { .. } | facts::PlaceSegment::FixedIndex { .. }
                )
            })
    {
        return false;
    }
    let Some((machine, _)) = validation::find_state(program, target_symbol) else {
        return false;
    };
    let aliases = parameter_symbols_for_storage(program, machine, parameter);
    let mut saw_store = false;
    for state_symbol in program
        .machine_states(machine)
        .iter()
        .map(|state| state.symbol)
    {
        let Some(state) = program.state_by_symbol(state_symbol) else {
            return false;
        };
        let Some(stored) = program
            .state_parameters(state)
            .iter()
            .find(|candidate| aliases.contains(&candidate.symbol))
        else {
            continue;
        };
        match state_keeps_byte_class_after_element_stores(
            program,
            semantic,
            build,
            state,
            stored.symbol,
            required,
        ) {
            ElementStoreClass::NoStore => {}
            ElementStoreClass::Proved => saw_store = true,
            ElementStoreClass::Retired => return false,
        }
    }
    saw_store
}

enum ElementStoreClass {
    NoStore,
    Proved,
    Retired,
}

/// Parameters that name the same storage as `parameter` after ordinary
/// transitions pass that parameter through as a whole argument.
fn parameter_symbols_for_storage(
    program: &dyn ::validation::CheckingInput,
    machine: &typed_trees::machine::Machine,
    parameter: SymbolHandle,
) -> Vec<SymbolHandle> {
    let mut aliases = vec![parameter];
    loop {
        let mut grew = false;
        let state_symbols: Vec<SymbolHandle> = program
            .machine_states(machine)
            .iter()
            .map(|state| state.symbol)
            .collect();
        for state_symbol in state_symbols {
            let Some(state) = program.state_by_symbol(state_symbol) else {
                continue;
            };
            for (_, statement) in program
                .statement_table
                .indexed_statements(state.statement_nodes)
            {
                let Some(transition) = statement.as_transition() else {
                    continue;
                };
                if transition.exit != typed_trees::statement::TransitionExit::Ordinary {
                    continue;
                }
                for target in transition.targets() {
                    let Some((path, arguments)) =
                        program.statement_table.named_transition_target(target)
                    else {
                        continue;
                    };
                    let Some(target_index) =
                        validation::internal_state_index(program, machine, path.symbol)
                    else {
                        continue;
                    };
                    let Some(destination_symbol) = program
                        .machine_states(machine)
                        .get(target_index)
                        .map(|state| state.symbol)
                    else {
                        continue;
                    };
                    let Some(destination) = program.state_by_symbol(destination_symbol) else {
                        continue;
                    };
                    let arguments = program.statement_table.expression_handles(arguments);
                    for (destination_parameter, argument) in program
                        .state_parameters(destination)
                        .iter()
                        .zip(arguments.iter().copied())
                    {
                        let Some(source) = whole_parameter_symbol(program, argument) else {
                            continue;
                        };
                        if aliases.contains(&source)
                            && !aliases.contains(&destination_parameter.symbol)
                        {
                            aliases.push(destination_parameter.symbol);
                            grew = true;
                        }
                    }
                }
            }
        }
        if !grew {
            break;
        }
    }
    aliases
}

fn whole_parameter_symbol(
    program: &dyn ::validation::CheckingInput,
    mut expression: typed_trees::expression::ExpressionHandle,
) -> Option<SymbolHandle> {
    loop {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                expression = borrow.target;
            }
            typed_trees::expression::ExpressionNode::Name(path) => return Some(path.symbol),
            _ => return None,
        }
    }
}

fn state_keeps_byte_class_after_element_stores(
    program: &dyn ::validation::CheckingInput,
    semantic: &FactPlan,
    build: &FlowBuildContext<'_>,
    state: &typed_trees::state::State,
    parameter: SymbolHandle,
    required: validation::field_domains::ByteSequencePredicate,
) -> ElementStoreClass {
    let mut last_store = None;
    for (statement_index, statement) in program
        .statement_table
        .indexed_statements(state.statement_nodes)
    {
        let Some(assignment) = statement.as_assignment() else {
            continue;
        };
        match carrier_assignment(program, assignment.target, parameter) {
            CarrierAssignment::Element => last_store = Some(statement_index),
            CarrierAssignment::Whole => return ElementStoreClass::Retired,
            CarrierAssignment::Other => {}
        }
    }
    let Some(last_store) = last_store else {
        return ElementStoreClass::NoStore;
    };
    let Some(flow) = build
        .control
        .states
        .iter()
        .find_map(|(_, flow)| (flow.state_symbol == state.symbol).then_some(flow))
    else {
        return ElementStoreClass::Retired;
    };
    let statement_span = flow.statements;
    let exit_span = flow.exits;
    let frontiers: Vec<HandleSpan<FlowSemanticContextRef>> = build
        .control
        .statements
        .span_or_empty(statement_span)
        .iter()
        .filter(|statement| statement.statement_index > last_store)
        .map(|statement| statement.entry_semantic_contexts)
        .collect();
    let exit_frontiers: Vec<HandleSpan<FlowSemanticContextRef>> = build
        .control
        .exits
        .span_or_empty(exit_span)
        .iter()
        .map(|exit| exit.entry_semantic_contexts)
        .collect();
    if frontiers.is_empty() && exit_frontiers.is_empty() {
        return ElementStoreClass::Retired;
    }
    let proves = |contexts: HandleSpan<FlowSemanticContextRef>| {
        build
            .contexts
            .semantic_context_refs
            .span_or_empty(contexts)
            .iter()
            .any(|reference| {
                semantic
                    .context_view(semantic.contexts.get(reference.context))
                    .facts()
                    .any(|fact| byte_class_fact_covers(semantic, fact, parameter, required))
            })
    };
    if frontiers.into_iter().chain(exit_frontiers).all(proves) {
        ElementStoreClass::Proved
    } else {
        ElementStoreClass::Retired
    }
}

fn byte_class_fact_covers(
    semantic: &FactPlan,
    fact: &Fact,
    parameter: SymbolHandle,
    required: validation::field_domains::ByteSequencePredicate,
) -> bool {
    let FactPayload::BytePredicate { predicate } = fact.payload else {
        return false;
    };
    if !predicate.implies(required) {
        return false;
    }
    let Some(place) = fact.place.as_place() else {
        return false;
    };
    let place = semantic.places.get(place);
    place.root.as_symbol() == Some(parameter)
        && semantic
            .place_segments
            .span_or_empty(place.segments)
            .iter()
            .all(|segment| {
                matches!(
                    segment,
                    facts::PlaceSegment::Field { .. }
                        | facts::PlaceSegment::Case { .. }
                        | facts::PlaceSegment::FixedIndex { .. }
                )
            })
}

enum CarrierAssignment {
    Element,
    Whole,
    Other,
}

fn carrier_assignment(
    program: &dyn ::validation::CheckingInput,
    mut expression: typed_trees::expression::ExpressionHandle,
    parameter: SymbolHandle,
) -> CarrierAssignment {
    let mut indexed = false;
    loop {
        match program.expression_table.expression(expression) {
            typed_trees::expression::ExpressionNode::Indexed(indexed_expression) => {
                indexed = true;
                expression = indexed_expression.collection;
            }
            typed_trees::expression::ExpressionNode::Borrow(borrow) => {
                expression = borrow.target;
            }
            typed_trees::expression::ExpressionNode::Name(path) if path.symbol == parameter => {
                return if indexed {
                    CarrierAssignment::Element
                } else {
                    CarrierAssignment::Whole
                };
            }
            _ => return CarrierAssignment::Other,
        }
    }
}
