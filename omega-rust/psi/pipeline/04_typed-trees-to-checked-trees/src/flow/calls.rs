use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
use crate::flow::append_call_boundary_edges;
use crate::flow::append_call_referent_field_domain_facts;
use crate::flow::append_constraint_ref;
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
    BorrowCallFact, BorrowFacts, DomainFacts, FlowCallFact, FlowConstraintKind, FlowConstraintRef,
    FlowSemanticContextRef, ProofFacts,
};
use facts::{
    Fact, FactOrigin, FactPayload, FactPlace, FactPlan, ProgramPoint, QualificationEvidence,
};
use symbols::SymbolHandle;

#[allow(clippy::too_many_arguments)]
pub(super) fn build_call_flow_fact<'plans>(
    program: &'plans typed_trees::TypedTrees,
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
        borrow,
        semantic,
        domains,
        build,
        machine,
        state,
        *active_contexts,
        *active_constraints,
        borrow_call,
    );
    let mut exit = build_call_exit_contexts(
        semantic,
        build,
        machine,
        state,
        borrow_call,
        invalidation.post_contexts,
        invalidation.post_constraints,
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
    );
    // Every readable `&mut` referent comes back satisfying the field facts
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
        receiver_symbol: borrow_call.receiver_symbol,
        target_symbol: borrow_call.target_symbol,
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
    program: &'plans typed_trees::TypedTrees,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    *build
        .call_target_returns
        .entry(target)
        .or_insert_with(|| call_target_return_type(program, target))
}

pub(super) fn memoized_call_target_parameters<'plans>(
    program: &'plans typed_trees::TypedTrees,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Option<&'plans [typed_trees::signature::StateParameter]> {
    *build
        .call_target_parameters
        .entry(target)
        .or_insert_with(|| crate::semantic::calls::call_target_parameters(program, target))
}

pub(super) fn memoized_find_call_site<'plans>(
    program: &'plans typed_trees::TypedTrees,
    build: &mut FlowBuildContext<'plans>,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<crate::semantic::calls::CallSite<'plans>> {
    *build
        .call_sites
        .entry((machine_symbol, state_symbol, statement_index, call_ordinal))
        .or_insert_with(|| {
            crate::semantic::calls::find_call_site(
                program,
                machine_symbol,
                state_symbol,
                statement_index,
                call_ordinal,
            )
        })
}

fn append_one_to_one_call_carry_facts<'plans>(
    program: &'plans typed_trees::TypedTrees,
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
    let Some(crate::semantic::calls::CallSite::Expression { expression, call }) =
        memoized_find_call_site(
            program,
            build,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )
    else {
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
        let argument = if parameter.is_self {
            call.receiver.is_valid().then_some(call.receiver)
        } else {
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
                FactPayload::CarryPermission { permission, .. }
                | FactPayload::ContractCarryPermission { permission, .. } => {
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
            let FactPlace::Place(fact_place) = fact.place else {
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
    let context = semantic.append_context(point, refs);
    reference_spans::append_flow_reference(
        &mut build.contexts.semantic_context_refs,
        &mut exit.contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut build.contexts.constraint_refs,
        &mut exit.constraints,
        FlowConstraintKind::SemanticContext { context },
    );
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
    program: &'plans typed_trees::TypedTrees,
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
    let Some(crate::semantic::calls::CallSite::Expression { expression, .. }) =
        memoized_find_call_site(
            program,
            build,
            machine.symbol,
            state.symbol,
            borrow_call.statement_index,
            borrow_call.call_ordinal,
        )
    else {
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
        let place = crate::semantic::places::append_place_with_segments(
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
    let context = semantic.append_context(point, refs);
    reference_spans::append_flow_reference(
        &mut build.contexts.semantic_context_refs,
        &mut exit.contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut build.contexts.constraint_refs,
        &mut exit.constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

/// Authored `ensures <parameter> in <domain>` claims the call hands back on
/// the exact argument place. These are provisional checked-signature facts,
/// not new issuance: the content checker independently rejoins routed
/// parameter claims to this invocation. An `ensures` subject naming an
/// immutable parameter asserts the caller's input still satisfies the
/// membership; naming a mutable parameter is the out-parameter establishment
/// the callee owed — either way the caller sees the same honest claim.
fn append_call_parameter_domain_facts<'plans>(
    program: &'plans typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    build: &mut FlowBuildContext<'plans>,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    exit: &mut CallFlowContexts,
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
    let arguments = crate::semantic::calls::call_site_argument_expressions(program, &site);
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
    for (position, domain_symbol, semantic_domain, declared) in claims.iter() {
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
            && !crate::facts::field_domain::domain_requires_provenance(program, *domain_symbol)
        {
            continue;
        }
        let Some(&argument) = arguments.get(*position) else {
            continue;
        };
        let Some(place) = crate::flow::canonical_place_from_expression_in_state(
            program,
            state.symbol,
            borrow_call.statement_index,
            argument,
        ) else {
            continue;
        };
        let place = crate::semantic::places::append_place_with_segments(
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
        semantic.append_ref(&mut refs, fact);
    }
    if refs.is_empty() {
        return;
    }
    let context = semantic.append_context(point, refs);
    reference_spans::append_flow_reference(
        &mut build.contexts.semantic_context_refs,
        &mut exit.contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut build.contexts.constraint_refs,
        &mut exit.constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

pub(crate) fn call_target_return_type(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(state) = crate::semantic::calls::find_state(program, target_state_symbol) {
        return Some(state.return_type);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(signature.return_type);
    }
    if let Some(reference) = asm_intrinsic_result_type(program, target_state_symbol) {
        return Some(reference);
    }
    program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_state_symbol)
            .map(|signature| signature.return_type)
    })
}

/// The asm-block value intrinsics (`in`, `rdmsr`, `pushfq`, `read_crN`,
/// `read_<sysreg>`) carry
/// fixed result types declared by the instruction contract rather than an
/// authored signature: their desugared calls target compiler builtin symbols,
/// which own no state, machine-parameter, or trait row to rejoin above. The
/// atom lookup keeps this a read over seeded symbols — a package cannot supply
/// a substitute signature for an unnameable intrinsic.
fn asm_intrinsic_result_type(
    program: &typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    let atom = match program.symbols.builtin_function_for_symbol(target)? {
        symbols::BuiltinFunction::AsmPortIn => symbols::BuiltinTypeAtom::U8,
        symbols::BuiltinFunction::AsmSnapshotFlags
        | symbols::BuiltinFunction::AsmReadMsr
        | symbols::BuiltinFunction::AsmReadCr0
        | symbols::BuiltinFunction::AsmReadCr2
        | symbols::BuiltinFunction::AsmReadCr3
        | symbols::BuiltinFunction::AsmReadCr4
        | symbols::BuiltinFunction::AsmReadSctlrEl1
        | symbols::BuiltinFunction::AsmReadTcrEl1
        | symbols::BuiltinFunction::AsmReadTtbr0El1
        | symbols::BuiltinFunction::AsmReadTtbr1El1
        | symbols::BuiltinFunction::AsmReadMairEl1
        | symbols::BuiltinFunction::AsmReadVbarEl1
        | symbols::BuiltinFunction::AsmReadTpidrEl1
        | symbols::BuiltinFunction::AsmReadEsrEl1
        | symbols::BuiltinFunction::AsmReadFarEl1 => symbols::BuiltinTypeAtom::U64,
        _ => return None,
    };
    let symbol = program
        .symbols
        .child_handles(program.symbols.root())?
        .find(|candidate| program.symbols.builtin_type_atom(*candidate) == Some(atom))?;
    program
        .type_reference_table
        .find_named_type_reference(symbol)
}

/// The signature contracts a callable's ensures/requires clauses live on: its
/// state contracts, the entry machine's contracts when the callable is an
/// entry state, and the trait signature's contracts when the callable is a
/// bodyless requirement. State and signature symbols are globally unique, so
/// each source contributes at most once.
fn collect_callable_contracts<'plans>(
    program: &'plans typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Vec<&'plans typed_trees::signature::SignatureContract> {
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
        for signature in program.trait_machine_signatures(owner) {
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
    program: &'plans typed_trees::TypedTrees,
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
    if let Some((owner_index, signature_index)) = build.signature_location(program, target) {
        let owner = &program.traits()[owner_index];
        let signature = &program.trait_machine_signatures(owner)[signature_index];
        contracts.extend(program.state_signature_contracts(signature));
    }
    contracts
}

/// One result obligation vocabulary for the provisional publisher and its
/// independent custody consumer. A conservation primitive may state `result
/// in D` in ensures rather than on the carrier type; that is still an exact
/// result promise, not issuer authorization or an argument qualification.
pub(crate) fn call_result_qualification_identities(
    program: &typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let Some(return_type) = call_target_return_type(program, target) else {
        return Vec::new();
    };
    let parameters = crate::semantic::calls::call_target_parameters(program, target);
    let contracts = collect_callable_contracts(program, target);
    result_identity_rows(program, return_type, parameters, &contracts)
}

/// `call_result_qualification_identities` on the context's memoized lookups
/// and symbol indexes -- identical rows, computed without the whole-program
/// scans. Used inside `call_result_identities`, so each target pays the
/// indexed lookup at most once per build.
fn call_result_qualification_identities_indexed<'plans>(
    program: &'plans typed_trees::TypedTrees,
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
    result_identity_rows(program, return_type, parameters, &contracts)
}

fn result_identity_rows(
    program: &typed_trees::TypedTrees,
    return_type: typed_trees::types::TypeReferenceHandle,
    parameters: Option<&[typed_trees::signature::StateParameter]>,
    contracts: &[&typed_trees::signature::SignatureContract],
) -> Vec<(
    Vec<facts::PlaceSegment>,
    SymbolHandle,
    language_semantics::SemanticDomainId,
)> {
    let mut carrier = return_type;
    while let typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } =
        program.type_reference_table.type_reference(carrier)
    {
        carrier = *base_type;
    }
    if matches!(
        program.type_reference_table.type_reference(carrier),
        typed_trees::types::TypeReferenceNode::Reference { .. }
    ) {
        return Vec::new();
    }
    let mut domains =
        crate::facts::field_domain::declared_owned_field_domain_identities(program, return_type);
    domains.extend(
        crate::facts::field_domain::domain_constraint_identities(program, return_type)
            .into_iter()
            .map(|(symbol, identity)| (Vec::new(), symbol, identity)),
    );
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Membership(membership) = fact else {
                continue;
            };
            let typed_trees::expression::ExpressionNode::Name(result) =
                program.expression_table.expression(membership.value)
            else {
                continue;
            };
            // This occurrence is already selected from this exact callable's
            // ensures, including bodyless trait requirements. It must be the
            // reserved result form, never a same-spelled parameter.
            if result.symbol.is_valid()
                || result.head_symbol.is_valid()
                || !matches!(program.expression_table.name_path_members(result.members), [name] if name.as_str() == "result")
                || parameters.is_some_and(|parameters| {
                    parameters
                        .iter()
                        .any(|parameter| parameter.name.as_str() == "result")
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
    domains
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
    program: &typed_trees::TypedTrees,
    target: SymbolHandle,
) -> Vec<(
    usize,
    SymbolHandle,
    language_semantics::SemanticDomainId,
    bool,
)> {
    let Some(parameters) = crate::semantic::calls::call_target_parameters(program, target) else {
        return Vec::new();
    };
    let contracts = collect_callable_contracts(program, target);
    parameter_identity_rows(program, parameters, &contracts)
}

/// `call_parameter_qualification_identities` on the context's memoized
/// lookups and symbol indexes, memoized per target inside
/// `call_parameter_identities`.
fn call_parameter_qualification_identities_indexed<'plans>(
    program: &'plans typed_trees::TypedTrees,
    build: &mut FlowBuildContext<'plans>,
    target: SymbolHandle,
) -> Vec<(usize, SymbolHandle, language_semantics::SemanticDomainId)> {
    let Some(parameters) = memoized_call_target_parameters(program, build, target) else {
        return Vec::new();
    };
    let contracts = collect_callable_contracts_indexed(program, build, target);
    parameter_identity_rows(program, parameters, &contracts)
}

fn parameter_identity_rows(
    program: &typed_trees::TypedTrees,
    parameters: &[typed_trees::signature::StateParameter],
    contracts: &[&typed_trees::signature::SignatureContract],
) -> Vec<(usize, SymbolHandle, language_semantics::SemanticDomainId)> {
    let mut rows = Vec::new();
    for contract in contracts
        .iter()
        .filter(|contract| contract.kind == typed_trees::signature::SignatureContractKind::Ensures)
    {
        for fact in program.proof_facts.span_or_empty(contract.facts) {
            let typed_trees::domain::ProofFact::Membership(membership) = fact else {
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
        if parameter.is_self {
            continue;
        }
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
            crate::facts::field_domain::domain_constraint_identities(
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
    program: &typed_trees::TypedTrees,
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
    if name.as_str() == "result" {
        return None;
    }
    parameters
        .iter()
        .filter(|parameter| !parameter.is_self)
        .position(|parameter| {
            parameter.name.as_str() == name.as_str()
                || parameter.symbol == path.symbol
                || parameter.symbol == path.head_symbol
        })
}
