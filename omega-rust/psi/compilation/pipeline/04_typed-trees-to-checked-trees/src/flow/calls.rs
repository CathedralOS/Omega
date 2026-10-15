//! One call's flow fact. `build_call_flow_fact` joins the call into the callee's
//! incoming values, builds entry, requires, invalidation, and exit contexts
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
use crate::flow::proof_contract_call;
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
    storage_writes: Option<&[validation::CanonicalPlace]>,
) -> FlowCallFact {
    super::state_values::record_invocation(program, build, machine, state, borrow_call);
    let contract_call = proof_contract_call(
        proof,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let entry = build_call_entry_contexts(
        borrow,
        build,
        *active_contexts,
        *active_constraints,
        machine.symbol,
        state.symbol,
        borrow_call,
    );
    let requires = build_call_requires_contexts(semantic, build, machine, state, borrow_call);
    let invalidation = apply_call_invalidations(
        program,
        semantic,
        domains,
        build,
        *active_contexts,
        *active_constraints,
        borrow_call,
        storage_writes,
    );
    let argument_sources = super::call_phases::ArgumentSources::new(
        program,
        machine.symbol,
        state.symbol,
        borrow_call,
        storage_writes,
        &build.control,
        captured_operands,
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
    );
    let boundary_edges = append_call_boundary_edges(program, build, borrow_call);
    *active_contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, exit.contexts);
    *active_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, exit.constraints);

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
    // These are provisional checked-signature facts, not new issuance. The
    // content checker independently rejoins routed result claims to this
    // invocation after linear claim reconstruction; ordinary callee exits
    // must establish every qualification before CheckedTrees can be accepted.
    if !build
        .call_result_identities
        .contains_key(&borrow_call.target_symbol)
    {
        let computed = std::rc::Rc::new(call_result_qualification_identities_indexed(
            program,
            build,
            borrow_call.target_symbol,
        ));
        build
            .call_result_identities
            .insert(borrow_call.target_symbol, computed);
    }
    let paths = build
        .call_result_identities
        .get(&borrow_call.target_symbol)
        .expect("call-result identities are inserted on miss")
        .clone();
    if paths.is_empty() {
        return;
    }
    let Some(validation::CallSite::Expression { expression, .. }) = memoized_find_call_site(
        program,
        build,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) else {
        return;
    };
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
    let mut refs = HandleSpan::empty();
    for (path, domain_symbol, semantic_domain) in paths.iter() {
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
    if !build
        .call_parameter_identities
        .contains_key(&borrow_call.target_symbol)
    {
        let computed = std::rc::Rc::new(call_parameter_qualification_identities_indexed(
            program,
            build,
            borrow_call.target_symbol,
        ));
        build
            .call_parameter_identities
            .insert(borrow_call.target_symbol, computed);
    }
    let claims = build
        .call_parameter_identities
        .get(&borrow_call.target_symbol)
        .expect("call-parameter identities are inserted on miss")
        .clone();
    if claims.is_empty() {
        return;
    }
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
    for (position, domain_symbol, semantic_domain, declared) in claims.iter() {
        if !declared
            && !argument_sources.is_some_and(|sources| sources.observation_survives(*position, &[]))
        {
            continue;
        }
        // An ENSURED claim publishes only for a routed domain: that is the
        // provenance the checker rejoins, and a predicate-domain `ensures`
        // would only add an invalidatable fact the call never minted.
        //
        // A DECLARED claim is the other direction. It comes from a mutable
        // reference parameter's own type, so every write the callee makes
        // through it owes that domain and its entry required it -- the
        // caller's argument holds it again on return. Without this the
        // mutation retires the caller's fact and nothing restores it, so an
        // out parameter could never carry a domain at all.
        if !declared
            && !validation::field_domains::domain_requires_provenance(program, *domain_symbol)
        {
            continue;
        }
        let Some(&argument) = arguments.get(*position) else {
            continue;
        };
        let Some(place) = validation::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            borrow_call.statement_index,
            argument,
        ) else {
            continue;
        };
        let place = crate::semantic::places::append_place(semantic, place.root, &place.segments);
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
pub(crate) fn call_result_qualification_identities(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let Some(return_type) = call_target_return_type(program, target) else {
        return Vec::new();
    };
    let parameters = validation::call_target_parameters(program, target);
    let contracts = collect_callable_contracts(program, target);
    result_identity_rows(program, return_type, parameters, &contracts)
}

/// `call_result_qualification_identities` on the context's memoized lookups
/// and symbol indexes -- identical rows, computed without the whole-program
/// scans. Used inside `call_result_identities`, so each target pays the
/// indexed lookup at most once per build.
fn call_result_qualification_identities_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let Some(return_type) = memoized_call_target_return_type(program, build, target) else {
        return Vec::new();
    };
    let parameters = memoized_call_target_parameters(program, build, target);
    let contracts = collect_callable_contracts_indexed(program, build, target);
    if !result_type_carries_identity_rows(program, return_type) {
        return Vec::new();
    }
    let domain_rows = build
        .result_identity_domain_rows
        .entry(return_type)
        .or_insert_with(|| std::rc::Rc::new(result_type_domain_rows(program, return_type)))
        .clone();
    let mut domains = (*domain_rows).clone();
    append_ensures_result_rows(program, parameters, &contracts, &mut domains);
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
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    if !result_type_carries_identity_rows(program, return_type) {
        return Vec::new();
    }
    let mut domains = result_type_domain_rows(program, return_type);
    append_ensures_result_rows(program, parameters, contracts, &mut domains);
    domains
}

fn append_ensures_result_rows(
    program: &dyn ::validation::CheckingInput,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    contracts: &[&typed_trees::signature::SignatureContract],
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
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
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
/// The domains a call's parameters carry on RETURN, each with the parameter
/// position and whether the claim came from the parameter's declared type
/// rather than an authored `ensures`.
pub(crate) fn call_parameter_qualification_identities(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Vec<(
    usize,
    SymbolHandle,
    language_semantics::SemanticDomainId,
    bool,
)> {
    let Some(parameters) = validation::call_target_parameters(program, target) else {
        return Vec::new();
    };
    let contracts = collect_callable_contracts(program, target);
    parameter_identity_rows(program, parameters, &contracts)
}

/// `call_parameter_qualification_identities` on the context's memoized
/// lookups and symbol indexes, memoized per target inside
/// `call_parameter_identities`.
fn call_parameter_qualification_identities_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Vec<(
    usize,
    SymbolHandle,
    language_semantics::SemanticDomainId,
    bool,
)> {
    let Some(parameters) = memoized_call_target_parameters(program, build, target) else {
        return Vec::new();
    };
    let contracts = collect_callable_contracts_indexed(program, build, target);
    parameter_identity_rows(program, parameters, &contracts)
}

fn parameter_identity_rows(
    program: &dyn ::validation::CheckingInput,
    parameters: &[typed_trees::signature::StateParameter],
    contracts: &[&typed_trees::signature::SignatureContract],
) -> Vec<(
    usize,
    SymbolHandle,
    language_semantics::SemanticDomainId,
    bool,
)> {
    let mut rows = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let Some(membership) = fact.as_membership() else {
                continue;
            };
            let Some(position) = ensured_parameter_position(program, parameters, membership.value)
            else {
                continue;
            };
            let row = (
                position,
                membership.domain_symbol,
                membership.semantic_domain,
                false,
            );
            if !rows.contains(&row) {
                rows.push(row);
            }
        }
    }
    // A mutable reference parameter's DECLARED domain is the callee's
    // postcondition as much as its precondition: the write check owes it on
    // every write through the reference, and entry required it of the caller.
    // A shared reference cannot be written, so its declared domain says
    // nothing the caller did not already have to prove.
    let mut position = 0;
    for parameter in parameters {
        let argument_position = position;
        position += 1;
        if !matches!(
            program
                .type_reference_table
                .type_reference(parameter.type_reference),
            typed_trees::types::TypeReferenceNode::Reference { access, .. }
                if access.is_exclusive()
        ) {
            continue;
        }
        for (domain_symbol, semantic_domain) in
            validation::field_domains::domain_constraint_identities(
                program,
                parameter.type_reference,
            )
        {
            let row = (argument_position, domain_symbol, semantic_domain, true);
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
pub(crate) fn ensured_parameter_position(
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
