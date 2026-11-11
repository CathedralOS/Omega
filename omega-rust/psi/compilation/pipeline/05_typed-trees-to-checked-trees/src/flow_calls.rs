/*
    One call's flow fact. `build_call_flow_fact` records conservative invocation
    inputs, builds entry, requires, invalidation, and exit contexts
    (`call_phases`), adds carry and domain facts to the exit, then appends boundary
    edges. The functions after it answer flow qualifications; exact callable types
    are owned by validation.
*/

use validation::call_target_return_type;

use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
use crate::flow::append_call_boundary_edges;
use crate::flow::append_call_referent_field_domain_facts;
use crate::flow::apply_call_invalidations;
use crate::flow::build_call_entry_contexts;
use crate::flow::build_call_exit_contexts;
use crate::flow::build_call_requires_contexts;
use crate::flow::expressions;
use crate::flow::reference_spans;
use crate::flow::retained_constraint_refs;
use crate::flow::retained_flow_contexts;
use crate::published::places::place_at;
use arena::HandleSpan;
use checked_trees::expression::ExpressionHandle;
use checked_trees::{
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintRef,
    FlowSemanticContextRef, ProofFacts,
};
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan};
use symbols::SymbolHandle;

pub(super) struct BuiltCallFlow {
    pub call: FlowCallFact,
    pub storage_writes: Option<super::call_phases::CallStorageWrites>,
}

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
) -> BuiltCallFlow {
    let storage_writes = writes.map(|writes| writes.observation_places.as_slice());
    let (contract_call, entry, requires) = {
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
            entry.contexts,
        );
        let requires = build_call_requires_contexts(semantic, build, machine, state, borrow_call);
        (contract_call, entry, requires)
    };
    let (argument_sources, observed_writes, invalidation) = {
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
            state.symbol,
            borrow_call,
            observed_writes.as_ref(),
        );
        (argument_sources, observed_writes, invalidation)
    };
    let mut exit = {
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
        exit
    };
    {
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
    };
    {
        append_call_referent_field_domain_facts(
            program,
            borrow,
            semantic,
            build,
            machine,
            state,
            borrow_call,
            entry.contexts,
            &mut exit,
            argument_sources.as_ref(),
            domains,
            observed_writes
                .as_ref()
                .map(|writes| writes.contributions.as_slice()),
        );
    };
    let boundary_edges = {
        super::call_phases::append_instruction_effect(
            program,
            semantic,
            build,
            machine,
            state,
            borrow_call,
            &mut exit,
        );
        append_call_boundary_edges(program, build, borrow_call)
    };
    *active_contexts = retained_flow_contexts(&build.contexts.semantic_context_refs, exit.contexts);
    *active_constraints =
        retained_constraint_refs(&build.contexts.constraint_refs, exit.constraints);

    if ::validation::trace_flags::struct_trace() {
        eprintln!(
            "FLOW call-storage-writes caller={} state={} target={:?} statement={} ordinal={} complete={} summary={storage_writes:?} observed={:?}",
            machine.name,
            state.name,
            borrow_call.target_symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
            storage_writes.is_some(),
            observed_writes
                .as_ref()
                .map(|writes| &writes.observation_places),
        );
    }
    let mut normal_return_writes = HandleSpan::empty();
    for place in observed_writes
        .as_ref()
        .map(|writes| writes.observation_places.as_slice())
        .into_iter()
        .flatten()
    {
        build.append_origin_write(&mut normal_return_writes, place);
    }
    let call = FlowCallFact {
        statement_index: borrow_call.statement_index,
        call_ordinal: borrow_call.call_ordinal,
        invocation_storage: Default::default(),
        completed_storage: Default::default(),
        authored_expression: Default::default(),
        authored_site: Default::default(),
        authored_target: Default::default(),
        authored_arguments: HandleSpan::empty(),
        argument_evaluation_order: HandleSpan::empty(),
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
        normal_return_referents: HandleSpan::empty(),
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
    };
    BuiltCallFlow {
        call,
        storage_writes: observed_writes,
    }
}

#[allow(clippy::too_many_arguments)]
/*
    Preserve the independent carry entry across the P1a mapping whose complete
    owned frontier is exactly one scalar linear input and one scalar linear
    output. The original evidence stays attached; declared-domain membership is
    intentionally not copied, so qualification weakening cannot launder carry.
    Conditional aggregates and every n-ary shape wait for P1c path mappings.
*/
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
    let Some(source_place) =
        crate::published::semantic::places::canonical_place_to_fact_place_in_state(
            program,
            semantic,
            state.symbol,
            borrow_call.statement_index,
            *source_argument,
        )
    else {
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
            crate::shared::collections::push_unique(&mut transfers, (payload, fact.evidence));
        }
    }
    if transfers.is_empty() {
        return;
    }

    let point = crate::flow::call_ensures_point(machine, state, borrow_call);
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

/*
    Publish the declared field predicates an OWNED nominal call result carries.
    The callee's own exit already proved each declared path against the live
    returned place (checks/contracts/exits.rs), so the signature-level promise
    is evidence, not a type annotation restoring what a write retired. The facts
    root at this exact call-expression occurrence: assignment copy transport
    rebases them onto the destination local, and a later write to that local's
    field retires them like any storage-backed fact. A reference return
    (`-> &Row`/`-> &mut Row`) produces no result storage of its own; its fields
    stay proven through the borrowed source place and are deliberately absent
    here so a later source write still invalidates them.
*/
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
    let selected = program.requirement_call_signature(machine.symbol, call);
    let Some(return_type) = build.expression_result_type_at(program, machine, state, expression)
    else {
        return;
    };
    if validation::reference_shell(program, return_type).is_some() {
        return;
    }
    let key = (
        borrow_call.target_symbol,
        selected.map_or(0, |s| std::ptr::from_ref(s) as usize),
        return_type,
    );
    if !build.call_result_identities.contains_key(&key) {
        let computed = std::rc::Rc::new(call_result_qualification_identities_indexed(
            program,
            build,
            borrow_call.target_symbol,
            selected,
            return_type,
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
    let point = crate::flow::call_ensures_point(machine, state, borrow_call);
    let evidence = crate::published::facts::propagated_evidence(borrow_call.target_symbol);
    let realization = checked_realization_result_identities(program, build, machine.symbol, call);
    let rows = paths.iter().filter(|row| {
        result_identity_is_available(
            program,
            machine.symbol,
            expression,
            call,
            realization.as_deref().map(Vec::as_slice),
            row,
        )
    });
    super::call_results::append_memberships(
        semantic,
        build,
        exit,
        expression,
        point,
        evidence,
        rows.map(|row| &row.identity),
    );
}

/*
    Preserve the issuer fence for owned and borrowed normal-result publishers.
    A direct checked adapter owes the exact result row independently; its
    requirement's boundary-only authority cannot supply a missing promise.
*/
pub(in crate::flow) fn result_identity_is_available(
    program: &dyn ::validation::CheckingInput,
    caller: SymbolHandle,
    expression: typed_trees::expression::ExpressionHandle,
    call: &typed_trees::expression::TableCallExpression,
    realization: Option<&[validation::field_domains::FieldDomainContract]>,
    row: &validation::field_domains::FieldDomainContract,
) -> bool {
    let Some(guaranteed) = realization else {
        return true;
    };
    if guaranteed.iter().any(|own| own.identity == row.identity)
        || !program
            .domain_definition(row.identity.1)
            .is_some_and(|domain| {
                domain.establishment_routes.iter().any(|route| {
                    matches!(
                        route,
                        language_semantics::DomainEstablishmentRoute::BoundaryRequirement { .. }
                    )
                })
            })
    {
        return true;
    }
    let site = validation::CallSite::Expression { expression, call };
    guaranteed
        .iter()
        .filter(|own| own.identity.0 == row.identity.0 && own.identity.1 == row.identity.1)
        .any(|own| {
            match validation::selected_call_result_domain_matches(
                program,
                caller,
                &site,
                row.identity.1,
                &own.arguments,
                &row.arguments,
            ) {
                Ok(matches) => matches,
                Err(reason) => {
                    if ::validation::trace_flags::struct_trace() {
                        eprintln!("selected result domain correspondence: {reason}");
                    }
                    false
                }
            }
        })
}

/*
    The checked adapter's exact result obligations, when this is a direct call.

    Use the same subject, projection and normalized domain identities as the
    publisher. Scanning domain symbols alone would let `ensures input in D`
    authorize an unrelated `result in D`. An empty checked-body roster grants
    nothing; absence denotes a crossing or bodyless provider and adds no fence.
*/
pub(in crate::flow) fn checked_realization_result_identities<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    lexical_owner: SymbolHandle,
    call: &typed_trees::expression::TableCallExpression,
) -> Option<std::rc::Rc<Vec<validation::field_domains::FieldDomainContract>>> {
    let dispatch = call.static_requirement_dispatch.as_ref()?;
    program
        .machine_by_symbol(dispatch.realization_machine)
        .filter(|machine| machine.body_is_present)?;
    let selected = program.realization_call_signature(lexical_owner, call);
    let return_type = selected
        .map(|signature| signature.return_type)
        .or_else(|| memoized_call_target_return_type(program, build, call.target_symbol))?;
    let key = (
        call.target_symbol,
        selected.map_or(0, |s| std::ptr::from_ref(s) as usize),
        return_type,
    );
    if !build.call_result_identities.contains_key(&key) {
        let computed = std::rc::Rc::new(call_result_qualification_identities_indexed(
            program,
            build,
            call.target_symbol,
            selected,
            return_type,
        ));
        build.call_result_identities.insert(key, computed);
    }
    build.call_result_identities.get(&key).cloned()
}

/*
    Authored `ensures <parameter> in <domain>` claims the call hands back on
    the exact argument place. These are provisional checked-signature facts,
    not new issuance: the content checker independently rejoins routed
    parameter claims to this invocation. A copied-input guarantee can describe
    caller storage only while its captured contents survive. A mutable-formal
    guarantee describes the live output and remains publishable after writes.
*/
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
        borrow_call.target_symbol,
        selected.map_or(0, |s| std::ptr::from_ref(s) as usize),
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
    let point = crate::flow::call_ensures_point(machine, state, borrow_call);
    let evidence = crate::published::facts::propagated_evidence(borrow_call.target_symbol);
    for (position, domain_symbol, semantic_domain) in claims.iter() {
        if argument_sources.is_some_and(|sources| sources.referent_places(*position).is_some())
            || !argument_sources.is_some_and(|sources| sources.observation_survives(*position, &[]))
        {
            continue;
        }
        if !validation::field_domains::domain_requires_provenance(program, *domain_symbol) {
            continue;
        }
        let Some(&argument) = arguments.get(*position) else {
            continue;
        };
        let places = place_at(program, state.symbol, borrow_call.statement_index, argument)
            .into_iter()
            .collect::<Vec<_>>();
        for place in &places {
            let place = crate::published::semantic::places::append_place(
                semantic,
                place.root,
                &place.segments,
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
            let mut refs = HandleSpan::empty();
            semantic.append_ref(&mut refs, fact);
            reference_spans::append_semantic_context(semantic, build, exit, point, refs);
        }
    }
}

/*
    The selected signature's own contract roster, when the call names exactly
    one; the caller supplies the roster to use when it names none.
*/
fn selected_signature_contracts<'a>(
    program: &'a dyn ::validation::CheckingInput,
    selected: Option<&typed_trees::signature::StateSignature>,
) -> Option<Vec<&'a typed_trees::signature::SignatureContract>> {
    selected.map(|signature| {
        program
            .state_signature_contracts(signature)
            .iter()
            .collect()
    })
}

/*
    The signature contracts a callable's ensures/requires clauses live on: its
    state contracts, the entry machine's contracts when the callable is an
    entry state, and the trait signature's contracts when the callable is a
    bodyless requirement. State and signature symbols are globally unique, so
    each source contributes at most once. A machine head selects its exact
    entry contract without changing the invocation's source identity.
*/
fn collect_callable_contracts(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
) -> Vec<&typed_trees::signature::SignatureContract> {
    let mut contracts = Vec::new();
    let executable_target = validation::call_target_entry_symbol(program, target);
    for machine in program.machines() {
        for (position, state) in program.machine_states(machine).iter().enumerate() {
            if state.symbol != executable_target {
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

/*
    `collect_callable_contracts` through the context's symbol indexes: one
    map lookup each for the state and signature locations rather than the
    whole machines x states x trait-signatures walk.
*/
fn collect_callable_contracts_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Vec<&'plans typed_trees::signature::SignatureContract> {
    let mut contracts = Vec::new();
    let executable_target = validation::call_target_entry_symbol(program, target);
    if let Some((machine_index, state_index)) = build.state_location(program, executable_target) {
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

/*
    One result obligation vocabulary for the provisional publisher and its
    independent custody consumer. A conservation primitive may state `result
    in D` in ensures rather than on the carrier type; that is still an exact
    result promise, not issuer authorization or an argument qualification.
    Enumerating a selected public promise does not authenticate its provenance:
    the actual invoked declaration must justify each transported ensures fact.
*/
pub(crate) fn call_result_qualification_identities(
    program: &dyn ::validation::CheckingInput,
    target: SymbolHandle,
    selected: Option<&typed_trees::signature::StateSignature>,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    if !return_type.is_valid() {
        return Vec::new();
    }
    let parameters = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| validation::call_target_parameters(program, target));
    let contracts = selected_signature_contracts(program, selected)
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
    .into_iter()
    .map(|row| row.identity)
    .collect()
}

/*
    `call_result_qualification_identities` on the context's memoized lookups
    and symbol indexes -- identical rows, computed without the whole-program
    scans. Used inside `call_result_identities`, so each call occurrence pays
    the indexed lookup at most once per build. Two calls to one requirement
    can have different local index tuples and therefore different result facts.
*/
pub(in crate::flow) fn call_result_qualification_identities_indexed<'plans>(
    program: &'plans dyn ::validation::CheckingInput,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
    selected: Option<&'plans typed_trees::signature::StateSignature>,
    return_type: typed_trees::types::TypeReferenceHandle,
) -> Vec<validation::field_domains::FieldDomainContract> {
    if !return_type.is_valid() {
        return Vec::new();
    }
    let parameters = selected
        .map(|signature| program.state_signature_parameters(signature))
        .or_else(|| memoized_call_target_parameters(program, build, target));
    let contracts = selected_signature_contracts(program, selected)
        .unwrap_or_else(|| collect_callable_contracts_indexed(program, build, target));
    if !return_type.is_valid() {
        return Vec::new();
    }
    let domain_rows = build
        .result_identity_domain_rows
        .entry(return_type)
        .or_insert_with(|| {
            std::rc::Rc::new(super::call_results::type_domain_rows(program, return_type))
        })
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

fn result_identity_rows(
    program: &dyn ::validation::CheckingInput,
    return_type: typed_trees::types::TypeReferenceHandle,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    contracts: &[&typed_trees::signature::SignatureContract],
    qualification_contract: &validation::CallQualificationContract<'_>,
) -> Vec<validation::field_domains::FieldDomainContract> {
    if !return_type.is_valid() {
        return Vec::new();
    }
    let mut domains = super::call_results::type_domain_rows(program, return_type);
    append_ensures_result_rows(
        program,
        parameters,
        contracts,
        qualification_contract,
        &mut domains,
    );
    domains
}

fn visit_qualified_ensures(
    program: &dyn ::validation::CheckingInput,
    contracts: &[&typed_trees::signature::SignatureContract],
    qualification_contract: &validation::CallQualificationContract<'_>,
    mut visit: impl FnMut(&typed_trees::typed_trees::declarations::domain::ProofMembershipFact),
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
            visit(membership);
        }
    }
}

fn append_ensures_result_rows(
    program: &dyn ::validation::CheckingInput,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    contracts: &[&typed_trees::signature::SignatureContract],
    qualification_contract: &validation::CallQualificationContract<'_>,
    domains: &mut Vec<validation::field_domains::FieldDomainContract>,
) {
    visit_qualified_ensures(program, contracts, qualification_contract, |membership| {
        if !expressions(program).names_reserved_result(membership.value)
            || parameters.is_some_and(|parameters| {
                !language_core::signature_reserves_result(
                    parameters.iter().map(|parameter| parameter.name.as_str()),
                )
            })
        {
            return;
        }
        let row = (
            Vec::new(),
            membership.domain_symbol,
            membership.semantic_domain,
        );
        if !domains.iter().any(|existing| existing.identity == row) {
            domains.push(validation::field_domains::FieldDomainContract {
                identity: row,
                arguments: program
                    .type_reference_table
                    .type_reference_handles(membership.domain_arguments)
                    .to_vec(),
            });
        }
    });
}

/*
    The out-parameter obligation vocabulary for the provisional publisher and
    its independent custody consumer: `ensures <parameter> in D` rows on the
    callable's signature contracts, keyed by the named non-self parameter's
    argument position. The reserved `result` subject belongs to the result
    obligation vocabulary above and is not repeated here.
    Additional input annotations are not part of this output vocabulary.
*/
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
    let contracts = selected_signature_contracts(program, selected)
        .unwrap_or_else(|| collect_callable_contracts(program, target));
    let qualification_contract =
        validation::CallQualificationContract::prepare(program, target, selected);
    parameter_identity_rows(program, parameters, &contracts, &qualification_contract)
}

/*
    `call_parameter_qualification_identities` on the context's memoized
    lookups and symbol indexes, memoized per call occurrence inside
    `call_parameter_identities`.
*/
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
    let contracts = selected_signature_contracts(program, selected)
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
    visit_qualified_ensures(program, contracts, qualification_contract, |membership| {
        let Some(position) = ensured_parameter_position(program, parameters, membership.value)
        else {
            return;
        };
        let row = (
            position,
            membership.domain_symbol,
            membership.semantic_domain,
        );
        crate::shared::collections::push_unique(&mut rows, row);
    });
    rows
}

/*
    The non-self argument position an `ensures` membership subject names, or
    none when the subject is the reserved `result`, a projection, or no exact
    parameter of the callable.
*/
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
