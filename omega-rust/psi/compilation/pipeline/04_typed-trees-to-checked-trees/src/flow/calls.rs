//! One call's flow fact. `build_call_flow_fact` records conservative invocation
//! inputs, builds entry, requires, invalidation, and exit contexts
//! (`call_phases`), adds carry and domain facts to the exit, then appends boundary
//! edges. The functions after it answer flow qualifications; exact callable types
//! are owned by validation.
use validation::call_target_return_type;

use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
use crate::flow::append_call_boundary_edges;
use crate::flow::append_call_referent_field_domain_facts;
use crate::flow::apply_call_invalidations;
use crate::flow::build_call_entry_contexts;
use crate::flow::build_call_exit_contexts;
use crate::flow::build_call_requires_contexts;
use crate::flow::reference_spans;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintRef,
    FlowSemanticContextRef, ProofFacts,
};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_call_flow_fact<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    active_contexts: &mut arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut arena::HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
    captured_operands: &[checked_trees::FlowOperandFact],
    writes: Option<&super::call_phases::CallStorageWrites>,
) -> FlowCallFact {
    let storage_writes = writes.map(|writes| writes.observation_places.as_slice());
    super::state_values::record_invocation(program, build, machine, state, borrow_call);
    let contract_call = proof.contract_call_at(&(
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ));
    let entry = build_call_entry_contexts(
        borrow,
        build,
        *active_contexts,
        *active_constraints,
        machine.symbol,
        state.symbol,
        borrow_call,
    );
    super::state_values::lexical_regions::record(
        program,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        captured_operands,
        entry.constraints,
    );
    let requires = build_call_requires_contexts(semantic, build, machine, state, borrow_call);
    let argument_sources = super::call_phases::ArgumentSources::new(
        program,
        machine.symbol,
        state.symbol,
        borrow_call,
        storage_writes,
        &build.control,
        captured_operands,
    );
    let observed_writes =
        writes.map(|writes| writes.with_captured_observations(argument_sources.as_ref()));
    let invalidation = apply_call_invalidations(
        program,
        semantic,
        domains,
        build,
        *active_contexts,
        *active_constraints,
        borrow_call,
        observed_writes.as_ref(),
    );
    let mut exit = build_call_exit_contexts(
        program,
        proof,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        invalidation.post_contexts,
        invalidation.post_constraints,
        argument_sources.as_ref(),
    );
    append_one_to_one_call_carry_facts(
        program,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        &entry,
        &mut exit,
    );
    append_call_result_field_domain_facts(
        program,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        &mut exit,
    );
    append_call_parameter_domain_facts(
        program,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        &mut exit,
        argument_sources.as_ref(),
    );
    // Every exclusive borrowed referent comes back satisfying the field facts
    // the callee re-proved at its return (checks/contracts/exits).
    append_call_referent_field_domain_facts(
        program,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        entry.contexts,
        &mut exit,
        argument_sources.as_ref(),
        domains,
        writes.map(|writes| writes.contributions.as_slice()),
    );
    super::call_phases::append_instruction_effect(
        program,
        semantic,
        build,
        machine,
        state,
        borrow_call,
        &mut exit,
    );
    let boundary_edges = append_call_boundary_edges(program, build, borrow_call);
    *active_contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, exit.contexts);
    *active_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, exit.constraints);

    // Publish the same complete-or-unknown footprint used to retire facts.
    // Contract consumers must distinguish normal-return storage from copied
    // invocation inputs without rebuilding a wider signature-only write set.
    let mut normal_return_writes = HandleSpan::empty();
    for place in observed_writes
        .as_ref()
        .map(|writes| writes.observation_places.as_slice())
        .into_iter()
        .flatten()
    {
        let segments = build
            .control
            .operand_referent_segments
            .insert_many(place.segments.iter().copied());
        build.control.operand_origin_writes.append_to_span(
            &mut normal_return_writes,
            checked_trees::FlowOperandReferent {
                formation: arena::Handle::invalid(),
                root: place.root,
                segments,
            },
        );
    }
    FlowCallFact {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        authored_expression: Default::default(),
        authored_site: Default::default(),
        authored_target: Default::default(),
        authored_arguments: HandleSpan::empty(),
        authored_receiver: Default::default(),
        authored_result: Default::default(),
        authored_closing_invocation: false,
        authored_dispatch_role: Default::default(),
        authored_selected: false,
        operands: HandleSpan::empty(),
        receiver_symbol: borrow_call.receiver_symbol,
        target_symbol: borrow_call.target_symbol,
        static_requirement_dispatch: None,
        has_receiver: borrow_call.has_receiver,
        accesses: borrow_call.accesses,
        entry_semantic_contexts: entry.contexts,
        entry_constraints: entry.constraints,
        requires_contexts: requires.contexts,
        requires_constraints: requires.constraints,
        exit_semantic_contexts: exit.contexts,
        exit_constraints: exit.constraints,
        invalidations: invalidation.invalidations,
        normal_return_writes,
        normal_return_writes_complete: storage_writes.is_some(),
        boundary_edges,
        requires: contract_call
            .map(|call| call.requires)
            .unwrap_or_else(HandleSpan::empty),
        ensures: contract_call
            .map(|call| call.ensures)
            .unwrap_or_else(HandleSpan::empty),
        service_reach: Default::default(),
        suspension: Default::default(),
        blocking: Default::default(),
        operational_acknowledgement: Default::default(),
        authored_source_span: None,
        authored_source_custody_valid: false,
    }
}

#[allow(clippy::too_many_arguments)]
/// Preserve the independent carry entry across the P1a mapping whose complete
/// owned frontier is exactly one scalar linear input and one scalar linear
/// output. The original evidence stays attached; declared-domain membership is
/// intentionally not copied, so qualification weakening cannot launder carry.
/// Conditional aggregates and every n-ary shape wait for P1c path mappings.
fn memoized_call_target_return_type<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    *build
        .call_target_returns
        .entry(target)
        .or_insert_with(|| call_target_return_type(program, target))
}

pub(super) fn memoized_call_target_parameters<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Option<&'plans [typed_trees::signature::StateParameter]> {
    *build
        .call_target_parameters
        .entry(target)
        .or_insert_with(|| validation::call_target_parameters(program, target))
}

pub(super) fn memoized_find_call_site<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<validation::CallSite<'plans>> {
    *build
        .call_sites
        .entry((machine_symbol, state_symbol, statement_index, call_ordinal))
        .or_default()
        .site
        .get_or_insert_with(|| {
            validation::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            )
        })
}

fn append_one_to_one_call_carry_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    entry: &CallFlowContexts,
    exit: &mut CallFlowContexts,
) {
    let Some(target_return_type) =
        memoized_call_target_return_type(program, build, borrow_call.target_symbol)
    else {
        return;
    };
    if crate::checks::type_multiplicity(program, target_return_type)
        != language_semantics::Multiplicity::Linear
    {
        return;
    }
    let Some(validation::CallSite::Expression { expression, call }) = memoized_find_call_site(
        program,
        build,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };

    let arguments = program.expression_table.expression_handles(call.arguments);
    let mut argument_index = 0usize;
    let mut linear_inputs = Vec::new();
    let Some(parameters) =
        memoized_call_target_parameters(program, build, borrow_call.target_symbol)
    else {
        return;
    };
    for parameter in parameters {
        let argument = {
            let argument = arguments.get(argument_index).copied();
            argument_index = argument_index.saturating_add(1);
            argument
        };
        if crate::checks::type_carries_linear_obligation(program, parameter.type_reference) {
            if crate::checks::type_multiplicity(program, parameter.type_reference)
                != language_semantics::Multiplicity::Linear
            {
                // Conditional aggregate obligations need a path-indexed P1c
                // outcome mapping; they are not a scalar one-to-one input.
                return;
            }
            if let Some(argument) = argument {
                linear_inputs.push(argument);
            }
        }
    }
    let [source_argument] = linear_inputs.as_slice() else {
        return;
    };
    let Some(source_place) = crate::semantic::places::canonical_place_to_fact_place_in_state(
        program,
        semantic,
        state.symbol,
        borrow_call.statement_index,
        *source_argument,
    ) else {
        return;
    };
    let target_place = semantic.append_place_from_expression(program, expression);
    let source_label = program.expression_table.display_name(*source_argument);
    let context_handles = build
        .contexts
        .semantic_context_refs
        .span_or_empty(entry.contexts)
        .iter()
        .map(|context_ref| context_ref.context)
        .collect::<Vec<_>>();
    let mut transfers = Vec::new();

    for context_handle in context_handles {
        let context = semantic.contexts.get(context_handle);
        for fact in semantic.context_view(context).facts() {
            if fact.evidence.origin == language_semantics::QualificationEvidenceOrigin::None {
                continue;
            }
            let payload = match fact.payload {
                facts::carry_permission_pair_pattern!(permission, ..) => {
                    FactPayload::CarryPermission {
                        value: ExpressionHandle::invalid(),
                        permission,
                    }
                }
                FactPayload::CarryOrigin { .. } => FactPayload::CarryOrigin {
                    value: ExpressionHandle::invalid(),
                },
                _ => continue,
            };
            let Some(fact_place) = fact.place.as_place() else {
                continue;
            };
            let fact_label = semantic.place_label(program, fact_place);
            if !semantic.places_match(program, fact_place, source_place)
                && fact_label != source_label
            {
                continue;
            }
            if !transfers.contains(&(payload, fact.evidence)) {
                transfers.push((payload, fact.evidence));
            }
        }
    }
    if transfers.is_empty() {
        return;
    }

    let point = ProgramPoint::CallEnsures {
        machine_symbol: machine.symbol,
        state_symbol: state.symbol,
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
    };
    let mut refs = HandleSpan::empty();
    for (payload, evidence) in transfers {
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(target_place),
            point,
            origin: FactOrigin::CallEnsures,
            evidence,
            payload,
        });
        semantic.append_ref(&mut refs, fact);
    }
    reference_spans::append_semantic_context(semantic, build, exit, point, refs);
}

/// Publish the declared field predicates an OWNED nominal call result carries.
/// The callee's own exit already proved each declared path against the live
/// returned place (checks/contracts/exits.rs), so the signature-level promise
/// is evidence, not a type annotation restoring what a write retired. The facts
/// root at this exact call-expression occurrence: assignment copy transport
/// rebases them onto the destination local, and a later write to that local's
/// field retires them like any storage-backed fact. A reference return
/// (`-> &Row`/`-> &mut Row`) produces no result storage of its own; its fields
/// stay proven through the borrowed source place and are deliberately absent
/// here so a later source write still invalidates them.
fn append_call_result_field_domain_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    exit: &mut CallFlowContexts,
) {
    let Some(validation::CallSite::Expression { expression, call }) = memoized_find_call_site(
        program,
        build,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };
    let key = (machine.symbol, expression);
    // These are provisional checked-signature facts, not new issuance. The
    // content checker independently rejoins routed result claims to this
    // invocation after linear claim reconstruction; ordinary callee exits
    // must establish every qualification before CheckedTrees can be accepted.
    if !build.call_result_identities.contains_key(&key) {
        let computed = std::rc::Rc::new(call_result_qualification_identities_indexed(
            program,
            build,
            borrow_call.target_symbol,
            program.requirement_call_signature(machine.symbol, call),
        ));
        build.call_result_identities.insert(key, computed);
    }
    let paths = build
        .call_result_identities
        .get(&key)
        .expect("call-result identities are inserted on miss")
        .clone();
    if paths.is_empty() {
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
    // A call that statically realizes a boundary requirement in a checked
    // machine is a direct call to that adapter, not the admitted crossing.
    // `facts::qualification_evidence::call_contract_evidence` already refuses
    // the attached authorization for exactly that call, so this publisher must
    // not re-derive the same membership from the requirement signature. The
    // crossing itself (a call with no realization dispatch) keeps its rows, and
    // an exact result membership the adapter's own contract carries is kept
    // because its body owes it. A promise about an input, another projection,
    // or another domain application cannot stand in for this result row.
    let realization = checked_realization_result_identities(program, call);
    let mut refs = HandleSpan::empty();
    for row @ (path, domain_symbol, semantic_domain) in paths.iter() {
        if realization
            .as_ref()
            .is_some_and(|guaranteed| !guaranteed.contains(row))
            && program
                .domain_definition(*domain_symbol)
                .is_some_and(|domain| {
                    domain.establishment_routes.iter().any(|route| {
                        matches!(
                            route,
                            language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }
                        )
                    })
                })
        {
            continue;
        }
        let place = crate::semantic::places::append_place(
            semantic,
            facts::PlaceRoot::Expression(expression),
            path,
        );
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
            point,
            origin: FactOrigin::CallEnsures,
            evidence,
            payload: FactPayload::DomainMembership {
                value: ExpressionHandle::invalid(),
                domain: HandleSpan::empty(),
                domain_symbol: *domain_symbol,
                semantic_domain: *semantic_domain,
            },
        });
        semantic.append_ref(&mut refs, fact);
    }
    reference_spans::append_semantic_context(semantic, build, exit, point, refs);
}

/// The checked adapter's exact result obligations, when this is a direct call.
///
/// Use the same subject, projection and normalized domain identities as the
/// publisher. Scanning domain symbols alone would let `ensures input in D`
/// authorize an unrelated `result in D`. An empty checked-body roster grants
/// nothing; absence denotes a crossing or bodyless provider and adds no fence.
fn checked_realization_result_identities(
    program: &dyn ::validation::CheckingInput,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<
    Vec<(
        Vec<facts::PlaceSegment>,
        SymbolHandle,
        language_semantics::SemanticDomainId,
    )>,
> {
    let dispatch = call.static_requirement_dispatch.as_ref()?;
    program
        .machine_by_symbol(dispatch.realization_machine)
        .filter(|machine| machine.body_is_present)?;
    // The dispatch retains the selected declaration's template identity.
    // Monomorphization has already joined the executable target to that
    // selection. Read the actual target's substituted types and contracts:
    // template parameter identities cannot match a closed invocation's rows.
    Some(call_result_qualification_identities(
        program,
        call.target_symbol,
        None,
    ))
}

/// Authored `ensures <parameter> in <domain>` claims the call hands back on
/// the exact argument place. These are provisional checked-signature facts,
/// not new issuance: the content checker independently rejoins routed
/// parameter claims to this invocation. A copied-input guarantee can describe
/// caller storage only while its captured contents survive. A mutable-formal
/// guarantee describes the live output and remains publishable after writes.
fn append_call_parameter_domain_facts<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    exit: &mut CallFlowContexts,
    argument_sources: Option<&super::call_phases::ArgumentSources<'_, '_>>,
) {
    let Some(site) = memoized_find_call_site(
        program,
        build,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };
    let selected = match site {
        validation::CallSite::Expression { call, .. } => {
            program.requirement_call_signature(machine.symbol, call)
        }
        _ => None,
    };
    let key = (
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    if !build.call_parameter_identities.contains_key(&key) {
        let computed = std::rc::Rc::new(call_parameter_qualification_identities_indexed(
            program,
            build,
            borrow_call.target_symbol,
            selected,
        ));
        build.call_parameter_identities.insert(key, computed);
    }
    let claims = build
        .call_parameter_identities
        .get(&key)
        .expect("call-parameter identities are inserted on miss")
        .clone();
    if claims.is_empty() {
        return;
    }
    let arguments = validation::call_site_argument_expressions(program, &site);
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
    for (position, domain_symbol, semantic_domain) in claims.iter() {
        // Declared child obligations belong to call_phases::referents. This
        // publisher owns authored routed ensures, never input annotations.
        if argument_sources.is_some_and(|sources| sources.referent_places(*position).is_some())
            || !argument_sources.is_some_and(|sources| sources.observation_survives(*position, &[]))
        {
            continue;
        }
        // An ENSURED claim publishes only for a routed domain: that is the
        // provenance the checker rejoins, and a predicate-domain `ensures`
        // would only add an invalidatable fact the call never minted.
        if !validation::field_domains::domain_requires_provenance(program, *domain_symbol) {
            continue;
        }
        let Some(&argument) = arguments.get(*position) else {
            continue;
        };
        // Stable copied inputs keep their observation-survival rule. Borrowed
        // guarantees use the actual captured occurrence in referents.rs.
        let places = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            borrow_call.statement_index,
            argument,
        )
        .into_iter()
        .collect::<Vec<_>>();
        for place in &places {
            let place =
                crate::semantic::places::append_place(semantic, place.root, &place.segments);
            let fact = semantic.append_fact(Fact {
                place: FactPlace::Place(place),
                point,
                origin: FactOrigin::CallEnsures,
                evidence,
                payload: FactPayload::DomainMembership {
                    value: ExpressionHandle::invalid(),
                    domain: HandleSpan::empty(),
                    domain_symbol: *domain_symbol,
                    semantic_domain: *semantic_domain,
                },
            });
            let mut refs = HandleSpan::empty();
            semantic.append_ref(&mut refs, fact);
            reference_spans::append_semantic_context(semantic, build, exit, point, refs);
        }
    }
}

/// The signature contracts a callable's ensures/requires clauses live on: its
/// state contracts, the entry machine's contracts when the callable is an
/// entry state, and the trait signature's contracts when the callable is a
/// bodyless requirement. State and signature symbols are globally unique, so
/// each source contributes at most once.
fn collect_callable_contracts(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Vec<&typed_trees::signature::SignatureContract> {
    let mut contracts = Vec::new();
    for machine in program.machines() {
        for (position, state) in program.machine_states(machine).iter().enumerate() {
            if state.symbol != target {
                continue;
            }
            contracts.extend(program.state_contracts(state));
            if position == 0 {
                contracts.extend(program.machine_contracts(machine));
            }
        }
    }
    for owner in program.traits() {
        for signature in program.trait_machine_instances(owner) {
            if signature.symbol == target {
                contracts.extend(program.state_signature_contracts(signature));
            }
        }
    }
    contracts
}

/// `collect_callable_contracts` through the context's symbol indexes: one
/// map lookup each for the state and signature locations rather than the
/// whole machines x states x trait-signatures walk.
fn collect_callable_contracts_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Vec<&'plans typed_trees::signature::SignatureContract> {
    let mut contracts = Vec::new();
    if let Some((machine_index, state_index)) = build.state_location(program, target) {
        let machine = &program.machines()[machine_index];
        let state = &program.machine_states(machine)[state_index];
        contracts.extend(program.state_contracts(state));
        if state_index == 0 {
            contracts.extend(program.machine_contracts(machine));
        }
    }
    if let Some((_, signature)) = program.trait_machine_signature(target) {
        contracts.extend(program.state_signature_contracts(signature));
    }
    contracts
}

/// One result obligation vocabulary for the provisional publisher and its
/// independent custody consumer. A conservation primitive may state `result
/// in D` in ensures rather than on the carrier type; that is still an exact
/// result promise, not issuer authorization or an argument qualification.
/// Enumerating a selected public promise does not authenticate its provenance:
/// the actual invoked declaration must justify each transported ensures fact.
pub(crate) fn call_result_qualification_identities(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
    selected: Option<&typed_trees::signature::StateSignature>,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let Some(return_type) = selected
        .map(|signature| signature.return_type)
        .or_else(|| call_target_return_type(program, target))
    else {
        return Vec::new();
    };
    let parameters = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| validation::call_target_parameters(program, target));
    let contracts = selected
        .map(|signature| {
            program
                .state_signature_contracts(signature)
                .iter()
                .collect()
        })
        .unwrap_or_else(|| collect_callable_contracts(program, target));
    let qualification_contract =
        validation::CallQualificationContract::prepare(program, target, selected);
    result_identity_rows(
        program,
        return_type,
        parameters,
        &contracts,
        &qualification_contract,
    )
}

/// `call_result_qualification_identities` on the context's memoized lookups
/// and symbol indexes -- identical rows, computed without the whole-program
/// scans. Used inside `call_result_identities`, so each call occurrence pays
/// the indexed lookup at most once per build. Two calls to one requirement
/// can have different local index tuples and therefore different result facts.
fn call_result_qualification_identities_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
    selected: Option<&'plans typed_trees::signature::StateSignature>,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let Some(return_type) = selected
        .map(|signature| signature.return_type)
        .or_else(|| memoized_call_target_return_type(program, build, target))
    else {
        return Vec::new();
    };
    let parameters = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| memoized_call_target_parameters(program, build, target));
    let contracts = selected
        .map(|signature| {
            program
                .state_signature_contracts(signature)
                .iter()
                .collect()
        })
        .unwrap_or_else(|| collect_callable_contracts_indexed(program, build, target));
    if !result_type_carries_identity_rows(program, return_type) {
        return Vec::new();
    }
    let domain_rows = build
        .result_identity_domain_rows
        .entry(return_type)
        .or_insert_with(|| std::rc::Rc::new(result_type_domain_rows(program, return_type)))
        .clone();
    let mut domains = (*domain_rows).clone();
    let qualification_contract =
        validation::CallQualificationContract::prepare(program, target, selected);
    append_ensures_result_rows(
        program,
        parameters,
        &contracts,
        &qualification_contract,
        &mut domains,
    );
    domains
}

/// Whether a return type can produce identity rows at all: types whose
/// peeled carrier is a reference yield none, so every caller can skip the
/// row build without consulting the (return_type-keyed) domain rows.
fn result_type_carries_identity_rows(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> bool {
    let carrier = program
        .type_reference_table
        .unconstrained_type_reference(return_type);
    !matches!(
        program.type_reference_table.type_reference(carrier),
        typed_trees::types::TypeReferenceNode::Reference { .. }
    )
}

/// The return-type-driven identity rows: declared owned-field domains plus
/// type-level constraint domains. Independent of the callable's contracts,
/// so callers may memoize them per return type.
fn result_type_domain_rows(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let mut domains =
        validation::field_domains::declared_owned_field_domain_identities(program, return_type);
    // Normal return also proves predicate requirements on readable children,
    // including stored borrowed views. Transport that checked promise with the
    // completed result; the owned-only roster deliberately stops at references.
    // Routed memberships keep their independent owned provenance judgment.
    domains.extend(
        validation::field_domains::declared_result_field_domain_paths(program, return_type)
            .into_iter()
            .filter(|(_, domain, _)| {
                crate::checks::contracts::value_provable_domain(program, *domain)
            }),
    );
    domains.extend(
        validation::field_domains::domain_constraint_identities(program, return_type)
            .into_iter()
            .map(|(symbol, identity)| (Vec::new(), symbol, identity)),
    );
    domains
}

fn result_identity_rows(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    contracts: &[&typed_trees::signature::SignatureContract],
    qualification_contract: &validation::CallQualificationContract<'_>,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    if !result_type_carries_identity_rows(program, return_type) {
        return Vec::new();
    }
    let mut domains = result_type_domain_rows(program, return_type);
    append_ensures_result_rows(
        program,
        parameters,
        contracts,
        qualification_contract,
        &mut domains,
    );
    domains
}

fn append_ensures_result_rows(
    program: &dyn ::validation::CheckingInput,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    contracts: &[&typed_trees::signature::SignatureContract],
    qualification_contract: &validation::CallQualificationContract<'_>,
    domains: &mut Vec<(
        Vec<facts::PlaceSegment>,
        SymbolHandle,
        language_semantics::SemanticDomainId,
    )>,
) {
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
    {
        for (fact_handle, fact) in contract
            .facts
            .handles()
            .zip(program.proof_facts.span_or_empty(contract.facts))
        {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            if qualification_contract
                .fact_source(contract.kind.clone(), fact)
                .is_none()
                && !qualification_contract
                    .declares_membership_obligation(contract.kind.clone(), fact_handle)
            {
                continue;
            }
            // This occurrence is already selected from this exact callable's
            // ensures, including bodyless trait requirements. It must be the
            // reserved result form, never a same-spelled parameter.
            if !program
                .expression_table
                .names_reserved_result(membership.value)
                || parameters.is_some_and(|parameters| {
                    !language_core::signature_reserves_result(
                        parameters.iter().map(|parameter| parameter.name.as_str()),
                    )
                })
            {
                continue;
            }
            let row = (
                Vec::new(),
                membership.domain_symbol,
                membership.semantic_domain,
            );
            if !domains.contains(&row) {
                domains.push(row);
            }
        }
    }
}

/// The out-parameter obligation vocabulary for the provisional publisher and
/// its independent custody consumer: `ensures <parameter> in D` rows on the
/// callable's signature contracts, keyed by the named non-self parameter's
/// argument position. The reserved `result` subject belongs to the result
/// obligation vocabulary above and is not repeated here.
/// Additional input annotations are not part of this output vocabulary.
pub(crate) fn call_parameter_qualification_identities(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
    selected: Option<&typed_trees::signature::StateSignature>,
) -> Vec<(usize, SymbolHandle, language_semantics::SemanticDomainId)> {
    let Some(parameters) = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| validation::call_target_parameters(program, target))
    else {
        return Vec::new();
    };
    let contracts = selected
        .map(|signature| {
            program
                .state_signature_contracts(signature)
                .iter()
                .collect()
        })
        .unwrap_or_else(|| collect_callable_contracts(program, target));
    let qualification_contract =
        validation::CallQualificationContract::prepare(program, target, selected);
    parameter_identity_rows(program, parameters, &contracts, &qualification_contract)
}

/// `call_parameter_qualification_identities` on the context's memoized
/// lookups and symbol indexes, memoized per call occurrence inside
/// `call_parameter_identities`.
fn call_parameter_qualification_identities_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
    selected: Option<&'plans typed_trees::signature::StateSignature>,
) -> Vec<(usize, SymbolHandle, language_semantics::SemanticDomainId)> {
    let Some(parameters) = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| memoized_call_target_parameters(program, build, target))
    else {
        return Vec::new();
    };
    let contracts = selected
        .map(|signature| {
            program
                .state_signature_contracts(signature)
                .iter()
                .collect()
        })
        .unwrap_or_else(|| collect_callable_contracts_indexed(program, build, target));
    let qualification_contract =
        validation::CallQualificationContract::prepare(program, target, selected);
    parameter_identity_rows(program, parameters, &contracts, &qualification_contract)
}

fn parameter_identity_rows(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    contracts: &[&typed_trees::signature::SignatureContract],
    qualification_contract: &validation::CallQualificationContract<'_>,
) -> Vec<(usize, SymbolHandle, language_semantics::SemanticDomainId)> {
    let mut rows = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
    {
        for (fact_handle, fact) in contract
            .facts
            .handles()
            .zip(program.proof_facts.span_or_empty(contract.facts))
        {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            if qualification_contract
                .fact_source(contract.kind.clone(), fact)
                .is_none()
                && !qualification_contract
                    .declares_membership_obligation(contract.kind.clone(), fact_handle)
            {
                continue;
            }
            let Some(position) = ensured_parameter_position(program, parameters, membership.value)
            else {
                continue;
            };
            let row = (
                position,
                membership.domain_symbol,
                membership.semantic_domain,
            );
            if !rows.contains(&row) {
                rows.push(row);
            }
        }
    }
    rows
}

/// The non-self argument position an `ensures` membership subject names, or
/// none when the subject is the reserved `result`, a projection, or no exact
/// parameter of the callable.
fn ensured_parameter_position(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    value: typed_trees::expression::ExpressionHandle,
) -> Option<usize> {
    let typed_trees::expression::ExpressionNode::Name(path) =
        program.expression_table.expression(value)
    else {
        return None;
    };
    let [name] = program.expression_table.name_path_members(path.members) else {
        return None;
    };
    if program.expression_table.names_reserved_result(value) {
        return None;
    }
    parameters.iter().position(|parameter| {
        parameter.name.as_str() == name.as_str()
            || parameter.symbol == path.symbol
            || parameter.symbol == path.head_symbol
    })
}
