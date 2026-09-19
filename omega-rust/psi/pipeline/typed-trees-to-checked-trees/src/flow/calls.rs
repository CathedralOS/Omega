use crate::facts::field_domain::declared_result_field_domain_paths;
use crate::flow::CallFlowContexts;
use crate::flow::FlowBuildContext;
use crate::flow::append_call_boundary_edges;
use crate::flow::append_call_referent_field_domain_facts;
use crate::flow::append_constraint_ref;
use crate::flow::apply_call_invalidations;
use crate::flow::build_call_entry_contexts;
use crate::flow::build_call_exit_contexts;
use crate::flow::build_call_requires_contexts;
use crate::flow::common;
use crate::flow::proof_contract_call;
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
pub(super) fn build_call_flow_fact(
    program: &typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &mut FactPlan,
    domains: &DomainFacts,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    active_contexts: &mut arena::HandleSpan<FlowSemanticContextRef>,
    active_constraints: &mut arena::HandleSpan<FlowConstraintRef>,
    borrow_call: &BorrowCallFact,
) -> FlowCallFact {
    super::state_values::record_invocation(program, ctx, machine, state, borrow_call);
    let contract_call = proof_contract_call(
        proof,
        machine.symbol,
        state.symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    );
    let entry = build_call_entry_contexts(
        borrow,
        ctx,
        *active_contexts,
        *active_constraints,
        machine.symbol,
        state.symbol,
        borrow_call,
    );
    let requires = build_call_requires_contexts(semantic, ctx, machine, state, borrow_call);
    let invalidation = apply_call_invalidations(
        program,
        borrow,
        semantic,
        domains,
        ctx,
        machine,
        state,
        *active_contexts,
        *active_constraints,
        borrow_call,
    );
    let mut exit = build_call_exit_contexts(
        semantic,
        ctx,
        machine,
        state,
        borrow_call,
        invalidation.post_contexts,
        invalidation.post_constraints,
    );
    append_one_to_one_call_carry_facts(
        program,
        semantic,
        ctx,
        machine,
        state,
        borrow_call,
        &entry,
        &mut exit,
    );
    append_call_result_field_domain_facts(
        program,
        semantic,
        ctx,
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
        ctx,
        machine,
        state,
        borrow_call,
        entry.contexts,
        &mut exit,
    );
    let boundary_edges = append_call_boundary_edges(program, ctx, borrow_call);
    *active_contexts = retained_flow_contexts(&ctx.contexts.semantic_context_refs, exit.contexts);
    *active_constraints = retained_constraint_refs(&ctx.contexts.constraint_refs, exit.constraints);

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
fn append_one_to_one_call_carry_facts(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    entry: &CallFlowContexts,
    exit: &mut CallFlowContexts,
) {
    let Some(target_return_type) = call_target_return_type(program, borrow_call.target_symbol)
    else {
        return;
    };
    if crate::checks::type_multiplicity(program, target_return_type)
        != language_semantics::Multiplicity::Linear
    {
        return;
    }
    let Some(crate::semantic_calls::CallSite::Expression { expression, call }) =
        crate::semantic_calls::find_call_site(
            program,
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
        crate::semantic_calls::call_target_parameters(program, borrow_call.target_symbol)
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
    let Some(source_place) = crate::semantic_places::canonical_place_to_fact_place_in_state(
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
    let context_handles = ctx
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
            let fact_label = crate::labels::canonical_place_label(
                program,
                semantic,
                semantic.places.get(fact_place),
            );
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
    common::append_flow_reference(
        &mut ctx.contexts.semantic_context_refs,
        &mut exit.contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
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
fn append_call_result_field_domain_facts(
    program: &typed_trees::TypedTrees,
    semantic: &mut FactPlan,
    ctx: &mut FlowBuildContext,
    machine: &typed_trees::machine::Machine,
    state: &typed_trees::state::State,
    borrow_call: &BorrowCallFact,
    exit: &mut CallFlowContexts,
) {
    let Some(return_type) = call_target_return_type(program, borrow_call.target_symbol) else {
        return;
    };
    let mut paths = declared_result_field_domain_paths(program, return_type)
        .into_iter()
        .filter(|(_, symbol)| {
            !crate::facts::field_domain::domain_requires_provenance(program, *symbol)
        })
        .map(|(path, symbol)| (path, symbol, language_semantics::SemanticDomainId::NULL))
        .collect::<Vec<_>>();
    // These are provisional checked-signature facts, not new issuance. The
    // content checker independently rejoins routed result claims to this
    // invocation after linear claim reconstruction; ordinary callee exits
    // must establish every qualification before CheckedTrees can be accepted.
    paths.extend(
        call_result_qualification_identities(program, borrow_call.target_symbol)
            .into_iter()
            .filter(|(_, symbol, _)| {
                crate::facts::field_domain::domain_requires_provenance(program, *symbol)
            }),
    );
    if paths.is_empty() {
        return;
    }
    let Some(crate::semantic_calls::CallSite::Expression { expression, .. }) =
        crate::semantic_calls::find_call_site(
            program,
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
    for (path, domain_symbol, semantic_domain) in paths {
        let place = crate::semantic_places::append_place_with_segments(
            semantic,
            facts::PlaceRoot::Expression(expression),
            &path,
        );
        let fact = semantic.append_fact(Fact {
            place: FactPlace::Place(place),
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
        semantic.append_ref(&mut refs, fact);
    }
    let context = semantic.append_context(point, refs);
    common::append_flow_reference(
        &mut ctx.contexts.semantic_context_refs,
        &mut exit.contexts,
        FlowSemanticContextRef { context },
    );
    append_constraint_ref(
        &mut ctx.contexts.constraint_refs,
        &mut exit.constraints,
        FlowConstraintKind::SemanticContext { context },
    );
}

pub(crate) fn call_target_return_type(
    program: &typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<typed_trees::types::TypeReferenceHandle> {
    if let Some(state) = crate::semantic_calls::find_state(program, target_state_symbol) {
        return Some(state.return_type);
    }
    if let Some((_, signature)) = program.machine_parameter_signature(target_state_symbol) {
        return Some(signature.return_type);
    }
    program.traits().iter().find_map(|trait_definition| {
        program
            .trait_machine_signatures(trait_definition)
            .iter()
            .find(|signature| signature.symbol == target_state_symbol)
            .map(|signature| signature.return_type)
    })
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
    for contract in contracts
        .into_iter()
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
                || crate::semantic_calls::call_target_parameters(program, target).is_some_and(
                    |parameters| {
                        parameters
                            .iter()
                            .any(|parameter| parameter.name.as_str() == "result")
                    },
                )
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
