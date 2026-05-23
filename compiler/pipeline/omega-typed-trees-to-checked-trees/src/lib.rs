use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, NamePath};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowRootKind,
    BorrowWritableRootFact, CheckFacts, ContractCallFact, ContractExitFact, ContractProofFact,
    ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef, FlowCallFact,
    FlowExitFact, FlowFacts, FlowSemanticContextRef, FlowStateFact, InvariantFact, InvariantFacts,
    Program, ProofFactKind, ProofFacts, ProofObligationFact, ProofObligationOwner, StateBorrowFact,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_core::diagnostics::Diagnostic;
use omega_core::symbols::SymbolHandle;
use omega_facts::{
    ContractFactKind as SemanticContractFactKind, Fact, FactOrigin, FactPayload, FactPlace,
    FactPlan, FactRef, ProgramPoint, ProofObligationKind as SemanticProofObligationKind,
};
use std::collections::BTreeSet;

pub fn lower_typed_trees(
    program: omega_typed_trees::TypedTrees,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    omega_validation::validate_program(&program)?;

    let proof_plan = omega_proof::obligations::build_proof_plan(&program);
    omega_proof::checker::check_proof_plan(&proof_plan)?;
    let effects = omega_effects::infer_effects(&program);
    omega_validation::validate_effect_plan(&program, &effects)?;
    let borrow = build_borrow_facts(&program);
    let proof = build_proof_facts(&program, &proof_plan, &borrow);
    let invariants = build_invariant_facts(&program);
    let semantic = build_semantic_facts(&program, &proof);
    let flow = build_flow_facts(&program, &borrow, &proof, &semantic, &effects);
    let facts = CheckFacts {
        semantic,
        proof,
        borrow,
        invariants,
        effects,
        flow,
    };
    check_flow_call_contracts(&program, &facts)?;

    Ok(Program {
        typed: program,
        facts,
    })
}

fn check_flow_call_contracts(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
) -> Result<(), Vec<Diagnostic>> {
    let mut diagnostics = Vec::new();

    for (_, state_flow) in facts.flow.states.iter() {
        for call_flow in facts.flow.calls.span_or_empty(state_flow.calls) {
            check_call_requires(program, facts, state_flow, call_flow, &mut diagnostics);
        }
    }

    if diagnostics.is_empty() {
        Ok(())
    } else {
        Err(diagnostics)
    }
}

fn check_call_requires(
    program: &omega_typed_trees::TypedTrees,
    facts: &CheckFacts,
    state_flow: &FlowStateFact,
    call_flow: &FlowCallFact,
    diagnostics: &mut Vec<Diagnostic>,
) {
    let entry_contexts = facts
        .flow
        .semantic_context_refs
        .span_or_empty(call_flow.entry_semantic_contexts);
    for requires_context in facts
        .flow
        .semantic_context_refs
        .span_or_empty(call_flow.requires_contexts)
    {
        let context = facts.semantic.contexts.get(requires_context.context);
        for fact in facts.semantic.context_view(context).facts() {
            let satisfied = match fact.payload {
                FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                    let place = match fact.place {
                        FactPlace::Place(place) => place,
                        _ => {
                            diagnostics.push(Diagnostic::error(format!(
                                "cannot interpret requires contract for call {} from {}",
                                call_target_label(program, call_flow.target_symbol),
                                machine_name(program, state_flow.machine_symbol)
                            )));
                            continue;
                        }
                    };
                    entry_contexts.iter().any(|entry_context| {
                        let context = facts.semantic.contexts.get(entry_context.context);
                        context_proves_requirement_place_domain(
                            program,
                            &facts.semantic,
                            context,
                            place,
                            domain_symbol,
                        )
                    })
                }
                FactPayload::ContractBooleanExpression { expression, .. } => matches!(
                    program.expression_table.expression(expression),
                    ExpressionNode::Boolean(true)
                ),
                _ => true,
            };

            if !satisfied {
                diagnostics.push(Diagnostic::error(format!(
                    "cannot prove requires contract for call {} from {}: {}",
                    call_target_label(program, call_flow.target_symbol),
                    machine_name(program, state_flow.machine_symbol),
                    semantic_fact_requirement_label(program, &facts.semantic, fact)
                )));
            }
        }
    }
}

fn context_proves_requirement_place_domain(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    context: &omega_facts::FactContext,
    required_place: omega_facts::PlaceHandle,
    required_domain: SymbolHandle,
) -> bool {
    let required_label =
        canonical_place_label(program, semantic, semantic.places.get(required_place));
    semantic.context_view(context).facts().any(|fact| {
        let (fact_domain, fact_place) = match fact.payload {
            FactPayload::DomainMembership { domain_symbol, .. }
            | FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                let FactPlace::Place(place) = fact.place else {
                    return false;
                };
                (domain_symbol, place)
            }
            _ => return false,
        };

        semantic.domain_implies(fact_domain, required_domain)
            && (semantic.places_equal(fact_place, required_place)
                || canonical_place_label(program, semantic, semantic.places.get(fact_place))
                    == required_label)
    })
}

fn build_semantic_facts(program: &omega_typed_trees::TypedTrees, proof: &ProofFacts) -> FactPlan {
    let mut facts = omega_facts::build_definition_fact_plan(program);
    append_proof_obligation_semantic_facts(proof, &mut facts);
    append_contract_semantic_facts(program, proof, &mut facts);

    facts
}

fn append_proof_obligation_semantic_facts(proof: &ProofFacts, facts: &mut FactPlan) {
    for (_, obligation) in proof.obligations.iter() {
        let point = proof_obligation_point(obligation);
        facts.append_fact_context(Fact {
            place: FactPlace::Unknown,
            point,
            origin: FactOrigin::ProofObligation,
            payload: FactPayload::ProofObligation {
                kind: semantic_proof_obligation_kind(obligation.kind.clone()),
            },
        });
    }
}

fn append_contract_semantic_facts(
    program: &omega_typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
) {
    let mut semantic_handles = Vec::with_capacity(proof.contract_facts.len());

    for (contract_handle, contract) in proof.contract_facts.iter() {
        let point = contract_fact_point(contract);
        let place = contract_fact_place(program, facts, contract);
        let payload = semantic_contract_payload(program, contract);
        let fact = facts.append_fact_context(Fact {
            place,
            point,
            origin: contract_fact_origin(contract),
            payload,
        });
        let contract_index = usize::try_from(contract_handle.arena_index())
            .expect("contract fact handle index overflow");
        while semantic_handles.len() <= contract_index {
            semantic_handles.push(None);
        }
        semantic_handles[contract_index] = Some(fact);
    }

    for (_, call) in proof.contract_calls.iter() {
        let mut combined_ref_values = Vec::new();
        let mut requires = HandleSpan::empty();
        append_call_semantic_contract_refs(
            program,
            proof,
            facts,
            call,
            call.requires,
            FactOrigin::CallRequires,
            ProgramPoint::CallRequires {
                machine_symbol: call.caller_machine_symbol,
                state_symbol: call.caller_state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            },
            &mut requires,
        );
        combined_ref_values.extend(facts.refs.span_or_empty(requires).iter().copied());
        if !requires.is_empty() {
            facts.append_context(
                ProgramPoint::CallRequires {
                    machine_symbol: call.caller_machine_symbol,
                    state_symbol: call.caller_state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                },
                requires,
            );
        }

        let mut ensures = HandleSpan::empty();
        append_call_semantic_contract_refs(
            program,
            proof,
            facts,
            call,
            call.ensures,
            FactOrigin::CallEnsures,
            ProgramPoint::CallEnsures {
                machine_symbol: call.caller_machine_symbol,
                state_symbol: call.caller_state_symbol,
                statement_index: call.statement_index,
                call_ordinal: call.call_ordinal,
            },
            &mut ensures,
        );
        combined_ref_values.extend(facts.refs.span_or_empty(ensures).iter().copied());
        if !ensures.is_empty() {
            facts.append_context(
                ProgramPoint::CallEnsures {
                    machine_symbol: call.caller_machine_symbol,
                    state_symbol: call.caller_state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                },
                ensures,
            );
        }
        let mut refs = HandleSpan::empty();
        for fact_ref in combined_ref_values {
            facts.refs.append_to_span(&mut refs, fact_ref);
        }
        facts.append_symbol_set(call.target_machine_symbol, refs);
    }

    for (_, exit) in proof.contract_exits.iter() {
        let mut refs = HandleSpan::empty();
        append_semantic_contract_refs(proof, facts, &semantic_handles, exit.ensures, &mut refs);
        facts.append_context(
            ProgramPoint::Exit {
                machine_symbol: exit.machine_symbol,
                state_symbol: exit.state_symbol,
                statement_index: exit.statement_index,
            },
            refs,
        );
        facts.append_symbol_set(exit.machine_symbol, refs);
    }
}

fn append_call_semantic_contract_refs(
    program: &omega_typed_trees::TypedTrees,
    proof: &ProofFacts,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    source_refs: HandleSpan<ContractProofFactRef>,
    origin: FactOrigin,
    point: ProgramPoint,
    refs: &mut HandleSpan<FactRef>,
) {
    for source_ref in proof.contract_fact_refs.span_or_empty(source_refs) {
        let contract = proof.contract_facts.get(source_ref.fact);
        let place = instantiate_call_contract_place(program, facts, call, contract);
        let payload = semantic_contract_payload(program, contract);
        let fact = facts.append_fact(Fact {
            place,
            point,
            origin,
            payload,
        });
        facts.append_ref(refs, fact);
    }
}

fn append_semantic_contract_refs(
    proof: &ProofFacts,
    facts: &mut FactPlan,
    semantic_handles: &[Option<omega_facts::FactHandle>],
    source_refs: HandleSpan<ContractProofFactRef>,
    refs: &mut HandleSpan<FactRef>,
) {
    for source_ref in proof.contract_fact_refs.span_or_empty(source_refs) {
        let source_index = usize::try_from(source_ref.fact.arena_index())
            .expect("contract fact ref handle index overflow");
        let Some(Some(fact)) = semantic_handles.get(source_index) else {
            continue;
        };
        facts.append_ref(refs, *fact);
    }
}

fn instantiate_call_contract_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            if let Some(place) =
                instantiate_call_contract_expression_place(program, facts, call, *expression)
            {
                return FactPlace::Place(place);
            }
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            if let Some(place) = instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                membership.value,
            ) {
                return FactPlace::Place(place);
            }
        }
    }

    let original_place = contract_fact_place(program, facts, contract);
    let FactPlace::Place(original_place_handle) = original_place else {
        return original_place;
    };

    let Some(substitution) =
        call_contract_place_substitution(program, facts, call, original_place_handle)
    else {
        return original_place;
    };

    let original_place = *facts.places.get(original_place_handle);
    let original_segments: Vec<_> = facts
        .place_segments
        .span_or_empty(original_place.segments)
        .iter()
        .copied()
        .collect();

    let mut segments = substitution.segments;
    segments.extend(original_segments);
    FactPlace::Place(append_place_with_segments(
        facts,
        substitution.root,
        &segments,
    ))
}

fn instantiate_call_contract_expression_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    expression: ExpressionHandle,
) -> Option<omega_facts::PlaceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            instantiate_call_contract_expression_place(program, facts, call, *inner)
        }
        ExpressionNode::Name(path) => {
            instantiate_call_contract_name_path_place(program, facts, call, path)
        }
        ExpressionNode::Member(member) => {
            let receiver = instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                member.receiver,
            )?;
            let segment = omega_facts::PlaceSegment::Field {
                symbol: effective_member_symbol(program, member.receiver, member),
            };
            Some(append_place_segment(facts, receiver, segment))
        }
        ExpressionNode::Indexed(indexed) => {
            let receiver = instantiate_call_contract_expression_place(
                program,
                facts,
                call,
                indexed.collection,
            )?;
            let segment = omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            };
            Some(append_place_segment(facts, receiver, segment))
        }
        _ => None,
    }
}

fn instantiate_call_contract_name_path_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    path: &omega_typed_trees::expression::TableNamePath,
) -> Option<omega_facts::PlaceHandle> {
    let members = program.expression_table.name_path_members(path.members);
    let head = members.first()?.as_str();
    let call_site = find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_state = find_state(program, call.target_state_symbol)?;

    let mut place = if head == "self" {
        receiver_place_for_call(program, facts, call, &call_site)?
    } else {
        let mut argument_index = 0usize;
        let mut matched = None;
        for parameter in program.state_parameters(target_state) {
            if parameter.is_self {
                continue;
            }

            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);

            if parameter.name.as_str() == head {
                matched = argument.and_then(|expr| canonical_place_to_fact_place(program, facts, expr));
                break;
            }
        }
        matched?
    };

    let tail_count = members.len().saturating_sub(1);
    if tail_count == 0 {
        return Some(place);
    }

    let member_symbols = program.expression_table.name_path_member_symbols(path.member_symbols);
    for (offset, member_name) in members.iter().skip(1).enumerate() {
        let symbol = member_symbols
            .get(offset + 1)
            .copied()
            .filter(|symbol| symbol.is_valid())
            .or_else(|| resolve_place_member_symbol(program, facts, place, member_name.as_str()))
            .unwrap_or_else(SymbolHandle::invalid);
        place = append_place_segment(
            facts,
            place,
            omega_facts::PlaceSegment::Field { symbol },
        );
    }

    Some(place)
}

fn canonical_place_to_fact_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    expression: ExpressionHandle,
) -> Option<omega_facts::PlaceHandle> {
    let canonical = canonical_place_from_expression(program, expression)?;
    Some(append_place_with_segments(
        facts,
        canonical.root,
        &canonical.segments,
    ))
}

fn append_place_segment(
    facts: &mut FactPlan,
    base_place: omega_facts::PlaceHandle,
    segment: omega_facts::PlaceSegment,
) -> omega_facts::PlaceHandle {
    let place = *facts.places.get(base_place);
    let mut segments: Vec<_> = facts
        .place_segments
        .span_or_empty(place.segments)
        .iter()
        .copied()
        .collect();
    segments.push(segment);
    append_place_with_segments(facts, place.root, &segments)
}

fn resolve_place_member_symbol(
    program: &omega_typed_trees::TypedTrees,
    facts: &FactPlan,
    place: omega_facts::PlaceHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let place = facts.places.get(place);
    let base_symbol = fact_place_type_symbol(program, facts, place)?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == base_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    None
}

fn fact_place_type_symbol(
    program: &omega_typed_trees::TypedTrees,
    facts: &FactPlan,
    place: &omega_facts::Place,
) -> Option<SymbolHandle> {
    let mut current = match place.root {
        omega_facts::PlaceRoot::Symbol(symbol) => symbol_type_symbol(program, symbol)?,
        omega_facts::PlaceRoot::Expression(expression) => expression_type_symbol(program, expression)?,
        _ => return None,
    };

    for segment in facts.place_segments.span_or_empty(place.segments) {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                current = symbol_type_symbol(program, *symbol)?;
            }
            omega_facts::PlaceSegment::Index { .. } => {
                return None;
            }
        }
    }

    Some(current)
}

#[derive(Debug, Clone)]
struct ContractPlaceSubstitution {
    root: omega_facts::PlaceRoot,
    segments: Vec<omega_facts::PlaceSegment>,
}

fn call_contract_place_substitution(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    original_place_handle: omega_facts::PlaceHandle,
) -> Option<ContractPlaceSubstitution> {
    let original_place = *facts.places.get(original_place_handle);
    let omega_facts::PlaceRoot::Symbol(parameter_symbol) = original_place.root else {
        return None;
    };
    let call_site = find_call_site(
        program,
        call.caller_machine_symbol,
        call.caller_state_symbol,
        call.statement_index,
        call.call_ordinal,
    )?;
    let target_state = find_state(program, call.target_state_symbol)?;
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let parameter_matches = parameter.symbol == parameter_symbol
            || symbol_name(program, parameter_symbol) == parameter.name.as_str();
        let substitution_place = if parameter.is_self {
            if !parameter_matches {
                continue;
            }
            receiver_place_for_call(program, facts, call, &call_site)
        } else {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if !parameter_matches {
                continue;
            }
            argument.map(|expression| facts.append_place_from_expression(program, expression))
        }?;

        let place = *facts.places.get(substitution_place);
        let segments = facts
            .place_segments
            .span_or_empty(place.segments)
            .iter()
            .copied()
            .collect();
        return Some(ContractPlaceSubstitution {
            root: place.root,
            segments,
        });
    }

    if symbol_name(program, parameter_symbol) == "self" {
        let substitution_place = receiver_place_for_call(program, facts, call, &call_site)?;
        let place = *facts.places.get(substitution_place);
        let segments = facts
            .place_segments
            .span_or_empty(place.segments)
            .iter()
            .copied()
            .collect();
        return Some(ContractPlaceSubstitution {
            root: place.root,
            segments,
        });
    }

    None
}

fn append_place_with_segments(
    facts: &mut FactPlan,
    root: omega_facts::PlaceRoot,
    segments: &[omega_facts::PlaceSegment],
) -> omega_facts::PlaceHandle {
    let place = facts.append_place(omega_facts::Place {
        root,
        segments: HandleSpan::empty(),
    });
    for segment in segments {
        facts.push_place_segment(place, *segment);
    }
    place
}

enum CallSite<'program> {
    Statement(&'program omega_typed_trees::statement::TableCall),
    Expression(&'program omega_typed_trees::expression::TableCallExpression),
}

fn find_call_site<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<CallSite<'program>> {
    let state = find_state_in_machine(program, machine_symbol, state_symbol)?;
    let machine = machine_by_symbol(program, machine_symbol)?;

    for (current_statement_index, statement) in program
        .statement_table
        .statements(state.statement_nodes)
        .iter()
        .enumerate()
    {
        let mut current_ordinal = 0usize;
        if let Some(call_site) = find_call_site_in_statement(
            program,
            machine,
            state,
            statement,
            current_statement_index,
            statement_index,
            call_ordinal,
            &mut current_ordinal,
        ) {
            return Some(call_site);
        }
    }

    None
}

fn find_call_site_in_statement<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    statement: &'program StatementNode,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match statement {
        StatementNode::Assignment(assignment) => find_call_site_in_expression(
            program,
            machine,
            state,
            assignment.value,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        StatementNode::Call(call) => {
            let is_machine_call = statement_call_can_dispatch_to_machine(program, machine, state, call)
                || call.target_symbol.is_valid();
            if is_machine_call {
                if current_statement_index == target_statement_index
                    && *current_ordinal == target_call_ordinal
                {
                    return Some(CallSite::Statement(call));
                }
                *current_ordinal = current_ordinal.saturating_add(1);
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }

            None
        }
        StatementNode::Expression(expression) => find_call_site_in_expression(
            program,
            machine,
            state,
            *expression,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        StatementNode::LocalData(local_data) => {
            if !local_data.initial_value.is_valid() {
                return None;
            }
            find_call_site_in_expression(
                program,
                machine,
                state,
                local_data.initial_value,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard
                && let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    expression,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            {
                return Some(call_site);
            }

            if let Some(call_site) = find_call_site_in_transition_target(
                program,
                machine,
                state,
                transition.target,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            ) {
                return Some(call_site);
            }

            if transition.continuation.is_valid() {
                return find_call_site_in_transition_target(
                    program,
                    machine,
                    state,
                    transition.continuation,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                );
            }

            None
        }
    }
}

fn find_call_site_in_transition_target<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        TransitionTargetNode::Value(expression) => find_call_site_in_expression(
            program,
            machine,
            state,
            *expression,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => None,
    }
}

fn find_call_site_in_expression<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
    state: &'program omega_typed_trees::state::State,
    expression: ExpressionHandle,
    current_statement_index: usize,
    target_statement_index: usize,
    target_call_ordinal: usize,
    current_ordinal: &mut usize,
) -> Option<CallSite<'program>> {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *value,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        ExpressionNode::Binary(binary) => find_call_site_in_expression(
            program,
            machine,
            state,
            binary.left,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        )
        .or_else(|| {
            find_call_site_in_expression(
                program,
                machine,
                state,
                binary.right,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }),
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(program, call.receiver);
            let is_machine_call = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_valid()
                || receiver_can_dispatch_to_machine(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                )
                || call.target_symbol.is_valid();

            if is_machine_call {
                if current_statement_index == target_statement_index
                    && *current_ordinal == target_call_ordinal
                {
                    return Some(CallSite::Expression(call));
                }
                *current_ordinal = current_ordinal.saturating_add(1);
            }

            if call.receiver.is_valid()
                && let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    call.receiver,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                )
            {
                return Some(call_site);
            }

            for argument in program.expression_table.expression_handles(call.arguments) {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    *argument,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }

            None
        }
        ExpressionNode::Cast(cast) => find_call_site_in_expression(
            program,
            machine,
            state,
            cast.value,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        ExpressionNode::Indexed(indexed) => find_call_site_in_expression(
            program,
            machine,
            state,
            indexed.collection,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        )
        .or_else(|| {
            find_call_site_in_expression(
                program,
                machine,
                state,
                indexed.index,
                current_statement_index,
                target_statement_index,
                target_call_ordinal,
                current_ordinal,
            )
        }),
        ExpressionNode::Member(member) => find_call_site_in_expression(
            program,
            machine,
            state,
            member.receiver,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        ExpressionNode::Mutable(inner) => find_call_site_in_expression(
            program,
            machine,
            state,
            *inner,
            current_statement_index,
            target_statement_index,
            target_call_ordinal,
            current_ordinal,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                if let Some(call_site) = find_call_site_in_expression(
                    program,
                    machine,
                    state,
                    field.value,
                    current_statement_index,
                    target_statement_index,
                    target_call_ordinal,
                    current_ordinal,
                ) {
                    return Some(call_site);
                }
            }
            None
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => None,
    }
}

fn call_site_argument_expressions<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    call_site: &CallSite<'program>,
) -> &'program [ExpressionHandle] {
    match call_site {
        CallSite::Statement(call) => program.statement_table.expression_handles(call.arguments),
        CallSite::Expression(call) => program.expression_table.expression_handles(call.arguments),
    }
}

fn receiver_place_for_call(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    call: &ContractCallFact,
    call_site: &CallSite<'_>,
) -> Option<omega_facts::PlaceHandle> {
    match call_site {
        CallSite::Statement(statement) => {
            if let Some(members) = statement_call_receiver_members(program, statement) {
                if members
                    .first()
                    .is_some_and(|member| member.as_str() == "self")
                {
                    let caller_state = find_state_in_machine(
                        program,
                        call.caller_machine_symbol,
                        call.caller_state_symbol,
                    )?;
                    let self_parameter = program
                        .state_parameters(caller_state)
                        .iter()
                        .find(|parameter| parameter.is_self)?;
                    let mut place = facts.append_symbol_place(self_parameter.symbol);
                    for member in members.iter().skip(1) {
                        let symbol = resolve_place_member_symbol(
                            program,
                            facts,
                            place,
                            member.as_str(),
                        )
                        .or_else(|| statement.receiver_symbol.is_valid().then_some(statement.receiver_symbol))
                        .unwrap_or_else(SymbolHandle::invalid);
                        place = append_place_segment(
                            facts,
                            place,
                            omega_facts::PlaceSegment::Field { symbol },
                        );
                    }
                    return Some(place);
                }

                if let Some(path) = statement_call_receiver_path(program, statement) {
                    return Some(append_place_from_name_path(facts, &path));
                }
            }
            statement
                .receiver_symbol
                .is_valid()
                .then(|| facts.append_symbol_place(statement.receiver_symbol))
        }
        CallSite::Expression(statement) => {
            if statement.receiver.is_valid() {
                return Some(facts.append_place_from_expression(program, statement.receiver));
            }

            let caller_state = find_state_in_machine(
                program,
                call.caller_machine_symbol,
                call.caller_state_symbol,
            )?;
            let self_parameter = program
                .state_parameters(caller_state)
                .iter()
                .find(|parameter| parameter.is_self)?;
            Some(facts.append_symbol_place(self_parameter.symbol))
        }
    }
}

fn append_place_from_name_path(facts: &mut FactPlan, path: &NamePath) -> omega_facts::PlaceHandle {
    let place = facts.append_symbol_place(path.head_symbol());
    for symbol in path.member_symbols().iter().skip(1) {
        if symbol.is_valid() {
            facts.push_place_segment(place, omega_facts::PlaceSegment::Field { symbol: *symbol });
        }
    }
    place
}

fn find_state_in_machine<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    let machine = program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)?;
    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
}

fn find_state<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> Option<&'program omega_typed_trees::state::State> {
    program.machines().iter().find_map(|machine| {
        program
            .machine_states(machine)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })
}

fn semantic_fact_requirement_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    fact: &Fact,
) -> String {
    match fact.payload {
        FactPayload::ContractDomainMembership {
            domain_symbol,
            value,
            ..
        } => format!(
            "{} in {}",
            requirement_place_label(program, semantic, fact.place)
                .unwrap_or_else(|| program.expression_table.display_name(value)),
            symbol_name(program, domain_symbol)
        ),
        FactPayload::ContractBooleanExpression { expression, .. } => {
            program.expression_table.display_name(expression)
        }
        _ => "unsupported contract fact".to_owned(),
    }
}

fn requirement_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: FactPlace,
) -> Option<String> {
    let FactPlace::Place(place) = place else {
        return None;
    };
    Some(canonical_place_label(
        program,
        semantic,
        semantic.places.get(place),
    ))
}

fn canonical_place_label(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> String {
    let mut label = match place.root {
        omega_facts::PlaceRoot::Unknown => "unknown".to_owned(),
        omega_facts::PlaceRoot::Symbol(symbol) => symbol_name(program, symbol),
        omega_facts::PlaceRoot::Expression(expression) => {
            program.expression_table.display_name(expression)
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => {
            program.display_type_reference(type_reference)
        }
    };
    for segment in semantic.place_segments.span_or_empty(place.segments) {
        match segment {
            omega_facts::PlaceSegment::Field { symbol } => {
                label.push('.');
                label.push_str(&symbol_name(program, *symbol));
            }
            omega_facts::PlaceSegment::Index { expression } => {
                label.push('[');
                label.push_str(&program.expression_table.display_name(*expression));
                label.push(']');
            }
        }
    }
    label
}

fn machine_name(program: &omega_typed_trees::TypedTrees, machine_symbol: SymbolHandle) -> String {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
        .map(|machine| machine.name.to_string())
        .unwrap_or_else(|| format!("machine#{}", machine_symbol.arena_index()))
}

fn call_target_label(
    program: &omega_typed_trees::TypedTrees,
    state_symbol: SymbolHandle,
) -> String {
    find_state(program, state_symbol)
        .map(|state| state.name.to_string())
        .unwrap_or_else(|| format!("state#{}", state_symbol.arena_index()))
}

fn symbol_name(program: &omega_typed_trees::TypedTrees, symbol: SymbolHandle) -> String {
    for machine in program.machines() {
        if machine.symbol == symbol {
            return machine.name.to_string();
        }
        for state in program.machine_states(machine) {
            if state.symbol == symbol {
                return state.name.to_string();
            }
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return parameter.name.to_string();
                }
            }
        }
        for data in program.machine_owned_data(machine) {
            if data.symbol == symbol {
                return data.name.to_string();
            }
        }
    }
    for data in program.data_definitions() {
        if data.symbol == symbol {
            return data.name.to_string();
        }
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field) if field.symbol == symbol => {
                    return field.name.to_string();
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.symbol == symbol =>
                {
                    return variant.name.to_string();
                }
                _ => {}
            }
        }
    }
    for domain in program.domain_definitions() {
        if domain.symbol == symbol {
            return domain.name.to_string();
        }
    }
    for invariant in program.invariant_definitions() {
        if invariant.symbol == symbol {
            return invariant.name.to_string();
        }
    }
    format!("symbol#{}", symbol.arena_index())
}

fn proof_obligation_point(obligation: &ProofObligationFact) -> ProgramPoint {
    match obligation.owner {
        ProofObligationOwner::MachineState {
            machine_symbol,
            state_symbol,
        }
        | ProofObligationOwner::StateReturn {
            machine_symbol,
            state_symbol,
        } => ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            data_symbol: _,
        } => ProgramPoint::Machine { machine_symbol },
        ProofObligationOwner::StateParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol: _,
        }
        | ProofObligationOwner::CallParameter {
            machine_symbol,
            state_symbol,
            target_symbol: _,
            parameter_symbol: _,
        }
        | ProofObligationOwner::TransitionParameter {
            machine_symbol,
            state_symbol,
            parameter_symbol: _,
        } => ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        ProofObligationOwner::Unknown => ProgramPoint::Global,
    }
}

fn contract_fact_point(contract: &ContractProofFact) -> ProgramPoint {
    match contract.owner {
        ContractProofFactOwner::Machine { machine_symbol } => {
            ProgramPoint::Machine { machine_symbol }
        }
        ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol,
        } => ProgramPoint::State {
            machine_symbol,
            state_symbol,
        },
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => ProgramPoint::State {
            machine_symbol: owner_symbol,
            state_symbol,
        },
        ContractProofFactOwner::Unknown => ProgramPoint::Global,
    }
}

fn contract_fact_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            FactPlace::Place(facts.append_place_from_expression(program, *expression))
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            FactPlace::Place(facts.append_place_from_expression(program, membership.value))
        }
    }
}

fn contract_fact_origin(contract: &ContractProofFact) -> FactOrigin {
    match contract.owner {
        ContractProofFactOwner::Machine { machine_symbol }
        | ContractProofFactOwner::MachineState {
            machine_symbol,
            state_symbol: _,
        } => FactOrigin::MachineContract { machine_symbol },
        ContractProofFactOwner::StateSignature {
            owner_symbol,
            state_symbol,
        } => FactOrigin::StateSignatureContract {
            owner_symbol,
            state_symbol,
        },
        ContractProofFactOwner::Unknown => FactOrigin::Unknown,
    }
}

fn semantic_contract_payload(
    program: &omega_typed_trees::TypedTrees,
    contract: &ContractProofFact,
) -> FactPayload {
    let kind = semantic_contract_fact_kind(contract.kind);
    match program.proof_facts.get(contract.fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            FactPayload::ContractBooleanExpression {
                kind,
                fact: contract.fact,
                expression: *expression,
            }
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            FactPayload::ContractDomainMembership {
                kind,
                fact: contract.fact,
                value: membership.value,
                domain: membership.domain,
                domain_symbol: membership.domain_symbol,
            }
        }
    }
}

fn semantic_contract_fact_kind(kind: ContractProofFactKind) -> SemanticContractFactKind {
    match kind {
        ContractProofFactKind::Requires => SemanticContractFactKind::Requires,
        ContractProofFactKind::Ensures => SemanticContractFactKind::Ensures,
        ContractProofFactKind::Trusted => SemanticContractFactKind::Trusted,
    }
}

fn semantic_proof_obligation_kind(kind: ProofFactKind) -> SemanticProofObligationKind {
    match kind {
        ProofFactKind::BoundedAssignment => SemanticProofObligationKind::BoundedAssignment,
        ProofFactKind::BoundedCallArgument => SemanticProofObligationKind::BoundedCallArgument,
        ProofFactKind::BoundedInitializer => SemanticProofObligationKind::BoundedInitializer,
        ProofFactKind::BoundedStateReturn => SemanticProofObligationKind::BoundedStateReturn,
        ProofFactKind::BoundedValue => SemanticProofObligationKind::BoundedValue,
        ProofFactKind::BoundedTransitionArgument => {
            SemanticProofObligationKind::BoundedTransitionArgument
        }
        ProofFactKind::GuardedTransition => SemanticProofObligationKind::GuardedTransition,
    }
}

pub fn lower_typed_program(
    program: omega_typed_trees::TypedTrees,
) -> Result<Program, Vec<omega_core::diagnostics::Diagnostic>> {
    lower_typed_trees(program)
}

fn build_proof_facts(
    program: &omega_typed_trees::TypedTrees,
    proof_plan: &omega_proof::obligations::ProofPlan,
    borrow: &BorrowFacts,
) -> ProofFacts {
    let mut obligations = omega_core::arena::Arena::with_capacity(proof_plan.obligations.len());
    let mut contract_facts =
        omega_core::arena::Arena::with_capacity(estimated_contract_fact_capacity(program));

    for (_, obligation) in proof_plan.obligations.iter() {
        obligations.append(match obligation {
            omega_proof::obligations::ProofObligation::BoundedAssignment(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedAssignment,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedCallArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedCallArgument,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: ProofObligationOwner::CallParameter {
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        target_symbol: obligation.target_symbol,
                        parameter_symbol: obligation.parameter_symbol,
                    },
                }
            }
            omega_proof::obligations::ProofObligation::BoundedInitializer(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedInitializer,
                    machine_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    state_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    owner: proof_owner(&obligation.owner),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedStateReturn(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedStateReturn,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: ProofObligationOwner::StateReturn {
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                    },
                }
            }
            omega_proof::obligations::ProofObligation::BoundedValue(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedValue,
                    machine_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    state_symbol: omega_core::symbols::SymbolHandle::invalid(),
                    owner: proof_owner(&obligation.owner),
                }
            }
            omega_proof::obligations::ProofObligation::BoundedTransitionArgument(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::BoundedTransitionArgument,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: ProofObligationOwner::TransitionParameter {
                        machine_symbol: obligation.machine_symbol,
                        state_symbol: obligation.state_symbol,
                        parameter_symbol: obligation.parameter_symbol,
                    },
                }
            }
            omega_proof::obligations::ProofObligation::GuardedTransition(obligation) => {
                ProofObligationFact {
                    kind: ProofFactKind::GuardedTransition,
                    machine_symbol: obligation.machine_symbol,
                    state_symbol: obligation.state_symbol,
                    owner: state_owner(obligation.machine_symbol, obligation.state_symbol),
                }
            }
        });
    }

    for machine in program.machines() {
        append_machine_contract_facts(program, machine, &mut contract_facts);
        append_inherited_trait_contract_facts(program, machine, &mut contract_facts);
    }
    for trait_definition in program.traits() {
        append_state_signature_contract_facts(
            program,
            trait_definition.symbol,
            program.trait_machine_signatures(trait_definition),
            &mut contract_facts,
        );
    }
    for platform in program.platforms() {
        append_state_signature_contract_facts(
            program,
            platform.symbol,
            program.platform_state_signatures(platform),
            &mut contract_facts,
        );
    }
    let (mut contract_fact_refs, contract_calls) =
        build_contract_call_facts(program, borrow, &contract_facts);
    let contract_exits =
        build_contract_exit_facts(program, &contract_facts, &mut contract_fact_refs);

    ProofFacts {
        obligations,
        contract_facts,
        contract_fact_refs,
        contract_calls,
        contract_exits,
    }
}

fn estimated_contract_fact_capacity(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_contracts(machine)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>()
        })
        .chain(program.traits().iter().map(|trait_definition| {
            program
                .trait_machine_signatures(trait_definition)
                .iter()
                .map(|signature| {
                    program
                        .state_signature_contracts(signature)
                        .iter()
                        .map(|contract| contract.facts.len())
                        .sum::<usize>()
                })
                .sum::<usize>()
        }))
        .chain(program.platforms().iter().map(|platform| {
            program
                .platform_state_signatures(platform)
                .iter()
                .map(|signature| {
                    program
                        .state_signature_contracts(signature)
                        .iter()
                        .map(|contract| contract.facts.len())
                        .sum::<usize>()
                })
                .sum::<usize>()
        }))
        .chain(
            program
                .machines()
                .iter()
                .map(|machine| estimated_inherited_trait_contract_fact_capacity(program, machine)),
        )
        .sum()
}

fn append_machine_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
) {
    for contract in program.machine_contracts(machine) {
        for fact in fact_handles(contract.facts) {
            contract_facts.append(ContractProofFact {
                kind: contract_fact_kind(contract.kind),
                owner: ContractProofFactOwner::Machine {
                    machine_symbol: machine.symbol,
                },
                fact,
            });
        }
    }
}

fn append_state_signature_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    owner_symbol: SymbolHandle,
    signatures: &[omega_typed_trees::signature::StateSignature],
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
) {
    for signature in signatures {
        for contract in program.state_signature_contracts(signature) {
            for fact in fact_handles(contract.facts) {
                contract_facts.append(ContractProofFact {
                    kind: contract_fact_kind(contract.kind),
                    owner: ContractProofFactOwner::StateSignature {
                        owner_symbol,
                        state_symbol: signature.symbol,
                    },
                    fact,
                });
            }
        }
    }
}

fn append_inherited_trait_contract_facts(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
) {
    let mut visited_traits = Vec::new();
    for conformance in program.machine_trait_conformances(machine) {
        let Some(trait_definition) = trait_definition_by_symbol(program, conformance.symbol) else {
            continue;
        };
        append_trait_contract_facts_for_machine(
            program,
            machine,
            trait_definition,
            contract_facts,
            &mut visited_traits,
        );
    }
}

fn append_trait_contract_facts_for_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    contract_facts: &mut omega_core::arena::Arena<ContractProofFact>,
    visited_traits: &mut Vec<SymbolHandle>,
) {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return;
    }

    visited_traits.push(trait_definition.symbol);

    for signature in program.trait_machine_signatures(trait_definition) {
        let Some((target_machine_symbol, target_state_symbol)) =
            trait_requirement_state_symbols(program, machine, signature)
        else {
            continue;
        };

        for contract in program.state_signature_contracts(signature) {
            for fact in fact_handles(contract.facts) {
                contract_facts.append(ContractProofFact {
                    kind: contract_fact_kind(contract.kind),
                    owner: ContractProofFactOwner::MachineState {
                        machine_symbol: target_machine_symbol,
                        state_symbol: target_state_symbol,
                    },
                    fact,
                });
            }
        }
    }

    for requirement in program.trait_requirements(trait_definition) {
        let Some(required_trait) = trait_definition_by_symbol(program, requirement.symbol) else {
            continue;
        };
        append_trait_contract_facts_for_machine(
            program,
            machine,
            required_trait,
            contract_facts,
            visited_traits,
        );
    }

    visited_traits.pop();
}

fn estimated_inherited_trait_contract_fact_capacity(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> usize {
    let mut visited_traits = Vec::new();
    program
        .machine_trait_conformances(machine)
        .iter()
        .filter_map(|conformance| trait_definition_by_symbol(program, conformance.symbol))
        .map(|trait_definition| {
            estimated_trait_contract_fact_capacity_for_machine(
                program,
                machine,
                trait_definition,
                &mut visited_traits,
            )
        })
        .sum()
}

fn estimated_trait_contract_fact_capacity_for_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    trait_definition: &omega_typed_trees::trait_definition::TraitDefinition,
    visited_traits: &mut Vec<SymbolHandle>,
) -> usize {
    if visited_traits
        .iter()
        .any(|symbol| *symbol == trait_definition.symbol)
    {
        return 0;
    }

    visited_traits.push(trait_definition.symbol);

    let direct = program
        .trait_machine_signatures(trait_definition)
        .iter()
        .filter(|signature| trait_requirement_state_symbols(program, machine, signature).is_some())
        .map(|signature| {
            program
                .state_signature_contracts(signature)
                .iter()
                .map(|contract| contract.facts.len())
                .sum::<usize>()
        })
        .sum::<usize>();
    let inherited = program
        .trait_requirements(trait_definition)
        .iter()
        .filter_map(|requirement| trait_definition_by_symbol(program, requirement.symbol))
        .map(|required_trait| {
            estimated_trait_contract_fact_capacity_for_machine(
                program,
                machine,
                required_trait,
                visited_traits,
            )
        })
        .sum::<usize>();

    visited_traits.pop();
    direct.saturating_add(inherited)
}

fn trait_requirement_state_symbols(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    requirement: &omega_typed_trees::signature::StateSignature,
) -> Option<(SymbolHandle, SymbolHandle)> {
    trait_conformance_candidate_machines(program, machine)
        .into_iter()
        .find_map(|candidate| {
            program
                .machine_states(candidate)
                .iter()
                .find(|state| state.name == requirement.name)
                .map(|state| (candidate.symbol, state.symbol))
        })
}

fn trait_conformance_candidate_machines<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &'program omega_typed_trees::machine::Machine,
) -> Vec<&'program omega_typed_trees::machine::Machine> {
    let Some(attached_data) = machine.attached_data.as_ref() else {
        return vec![machine];
    };

    let mut candidates = Vec::new();
    candidates.push(machine);
    candidates.extend(program.machines().iter().filter(|candidate| {
        !std::ptr::eq(*candidate, machine)
            && candidate.attached_data.as_ref() == Some(attached_data)
    }));
    candidates
}

fn trait_definition_by_symbol(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::trait_definition::TraitDefinition> {
    if !symbol.is_valid() {
        return None;
    }

    program
        .traits()
        .iter()
        .find(|trait_definition| trait_definition.symbol == symbol)
}

fn build_contract_call_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
) -> (
    omega_core::arena::Arena<ContractProofFactRef>,
    omega_core::arena::Arena<ContractCallFact>,
) {
    let mut fact_refs = omega_core::arena::Arena::with_capacity(contract_facts.len());
    let mut calls = omega_core::arena::Arena::with_capacity(borrow.calls.len());

    for state in borrow.states.iter().map(|(_, state)| state) {
        for call in borrow.calls.span_or_empty(state.calls) {
            let Some((target_machine_symbol, target_state_symbol)) =
                contract_target_from_state_symbol(program, call.target_symbol)
            else {
                continue;
            };

            append_contract_call(
                contract_facts,
                &mut fact_refs,
                &mut calls,
                ContractCallSite {
                    caller_machine_symbol: state.machine_symbol,
                    caller_state_symbol: state.state_symbol,
                    statement_index: call.statement_index,
                    call_ordinal: call.call_ordinal,
                    target_machine_symbol,
                    target_state_symbol,
                },
            );
        }
    }

    (fact_refs, calls)
}

#[derive(Debug, Clone, Copy)]
struct ContractCallSite {
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
    target_machine_symbol: SymbolHandle,
    target_state_symbol: SymbolHandle,
}

fn append_contract_call(
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
    calls: &mut omega_core::arena::Arena<ContractCallFact>,
    site: ContractCallSite,
) {
    let requires = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.target_machine_symbol,
        Some(site.target_state_symbol),
        ContractProofFactKind::Requires,
    );
    let ensures = append_contract_fact_refs(
        contract_facts,
        fact_refs,
        site.target_machine_symbol,
        Some(site.target_state_symbol),
        ContractProofFactKind::Ensures,
    );

    if requires.is_empty() && ensures.is_empty() {
        return;
    }

    calls.append(ContractCallFact {
        caller_machine_symbol: site.caller_machine_symbol,
        caller_state_symbol: site.caller_state_symbol,
        statement_index: site.statement_index,
        call_ordinal: site.call_ordinal,
        target_machine_symbol: site.target_machine_symbol,
        target_state_symbol: site.target_state_symbol,
        requires,
        ensures,
    });
}

fn build_contract_exit_facts(
    program: &omega_typed_trees::TypedTrees,
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
) -> omega_core::arena::Arena<ContractExitFact> {
    let mut exits = omega_core::arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let statements = program.statement_table.statements(state.statement_nodes);
            let Some((statement_index, StatementNode::Expression(_))) =
                statements.iter().enumerate().next_back()
            else {
                continue;
            };
            let ensures = append_contract_fact_refs(
                contract_facts,
                fact_refs,
                machine.symbol,
                Some(state.symbol),
                ContractProofFactKind::Ensures,
            );

            if ensures.is_empty() {
                continue;
            }

            exits.append(ContractExitFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                statement_index,
                ensures,
            });
        }
    }

    exits
}

fn append_contract_fact_refs(
    contract_facts: &omega_core::arena::Arena<ContractProofFact>,
    fact_refs: &mut omega_core::arena::Arena<ContractProofFactRef>,
    machine_symbol: SymbolHandle,
    state_symbol: Option<SymbolHandle>,
    kind: ContractProofFactKind,
) -> HandleSpan<ContractProofFactRef> {
    let mut span = HandleSpan::empty();

    for (handle, fact) in contract_facts.iter() {
        let owner_matches = match fact.owner {
            ContractProofFactOwner::Machine {
                machine_symbol: owner_symbol,
            } => owner_symbol == machine_symbol,
            ContractProofFactOwner::MachineState {
                machine_symbol: owner_machine_symbol,
                state_symbol: owner_state_symbol,
            } => {
                owner_machine_symbol == machine_symbol
                    && state_symbol.is_some_and(|state_symbol| state_symbol == owner_state_symbol)
            }
            ContractProofFactOwner::Unknown | ContractProofFactOwner::StateSignature { .. } => {
                false
            }
        };

        if owner_matches && fact.kind == kind {
            fact_refs.append_to_span(&mut span, ContractProofFactRef { fact: handle });
        }
    }

    span
}

fn contract_target_from_state_symbol(
    program: &omega_typed_trees::TypedTrees,
    target_state_symbol: SymbolHandle,
) -> Option<(SymbolHandle, SymbolHandle)> {
    if !target_state_symbol.is_valid() {
        return None;
    }

    let target_machine = program.machines().iter().find(|machine| {
        program
            .machine_states(machine)
            .iter()
            .any(|state| state.symbol == target_state_symbol)
    })?;
    Some((target_machine.symbol, target_state_symbol))
}

fn fact_handles(
    facts: HandleSpan<omega_typed_trees::domain::ProofFact>,
) -> impl Iterator<Item = Handle<omega_typed_trees::domain::ProofFact>> {
    (0..facts.count()).map(move |offset| {
        Handle::from_parts(
            facts
                .start()
                .arena_index()
                .checked_add(offset)
                .expect("proof fact handle index overflow"),
            facts.start().generation(),
        )
    })
}

fn contract_fact_kind(
    kind: omega_typed_trees::signature::SignatureContractKind,
) -> ContractProofFactKind {
    match kind {
        omega_typed_trees::signature::SignatureContractKind::Requires => {
            ContractProofFactKind::Requires
        }
        omega_typed_trees::signature::SignatureContractKind::Ensures => {
            ContractProofFactKind::Ensures
        }
        omega_typed_trees::signature::SignatureContractKind::Trusted => {
            ContractProofFactKind::Trusted
        }
    }
}

fn state_owner(machine_symbol: SymbolHandle, state_symbol: SymbolHandle) -> ProofObligationOwner {
    ProofObligationOwner::MachineState {
        machine_symbol,
        state_symbol,
    }
}

fn proof_owner(owner: &omega_proof::obligations::ProofObligationOwner) -> ProofObligationOwner {
    match owner {
        omega_proof::obligations::ProofObligationOwner::Unknown => ProofObligationOwner::Unknown,
        omega_proof::obligations::ProofObligationOwner::MachineOwnedData {
            machine_symbol,
            machine: _,
            data_symbol,
            data: _,
        } => ProofObligationOwner::MachineOwnedData {
            machine_symbol: *machine_symbol,
            data_symbol: *data_symbol,
        },
        omega_proof::obligations::ProofObligationOwner::StateParameter {
            machine_symbol,
            machine: _,
            state_symbol,
            state: _,
            parameter_symbol,
            parameter: _,
        } => ProofObligationOwner::StateParameter {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
            parameter_symbol: *parameter_symbol,
        },
        omega_proof::obligations::ProofObligationOwner::StateReturn {
            machine_symbol,
            machine: _,
            state_symbol,
            state: _,
        } => ProofObligationOwner::StateReturn {
            machine_symbol: *machine_symbol,
            state_symbol: *state_symbol,
        },
    }
}

fn build_invariant_facts(program: &omega_typed_trees::TypedTrees) -> InvariantFacts {
    let mut definitions =
        omega_core::arena::Arena::with_capacity(program.invariant_definitions().len());

    for definition in program.invariant_definitions() {
        definitions.append(InvariantFact {
            symbol: definition.symbol,
            name: definition.name.clone(),
            constraint_count: program
                .type_reference_table
                .constraints(definition.constraints)
                .len(),
        });
    }

    InvariantFacts { definitions }
}

fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &FactPlan,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let mut domain_dependency_cache = DomainDependencyCache::default();
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let mut semantic_context_refs =
        omega_core::arena::Arena::with_capacity(semantic.contexts.len().saturating_mul(2));
    let mut calls = omega_core::arena::Arena::with_capacity(borrow.calls.len());
    let mut exits = omega_core::arena::Arena::with_capacity(proof.contract_exits.len());
    let mut states = omega_core::arena::Arena::with_capacity(borrow.states.len());

    for machine in program.machines() {
        let machine_effects = effects_machine(effects, machine.symbol);

        for state in program.machine_states(machine) {
            let Some(borrow_state) = borrow_state_fact(borrow, machine.symbol, state.symbol) else {
                continue;
            };
            let state_effects = effects_state(effects, machine_effects, state.symbol);
            let mut state_contexts = omega_core::arena::HandleSpan::empty();
            append_flow_contexts_for_points(
                semantic,
                &mut semantic_context_refs,
                &mut state_contexts,
                &[
                    ProgramPoint::Global,
                    ProgramPoint::Machine {
                        machine_symbol: machine.symbol,
                    },
                    ProgramPoint::State {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                    },
                ],
            );
            let mut active_contexts =
                clone_flow_contexts(&mut semantic_context_refs, state_contexts);

            let mut state_calls = omega_core::arena::HandleSpan::empty();
            let borrow_calls = borrow.calls.span_or_empty(borrow_state.calls);
            let mut call_index = 0usize;
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                while let Some(borrow_call) = borrow_calls.get(call_index) {
                    if borrow_call.statement_index != statement_index {
                        break;
                    }
                    call_index += 1;

                    let effect_call = effects_call(effects, state_effects, borrow_call);
                    let contract_call = proof_contract_call(
                        proof,
                        machine.symbol,
                        state.symbol,
                        borrow_call.statement_index,
                        borrow_call.call_ordinal,
                    );
                    let entry_contexts =
                        clone_flow_contexts(&mut semantic_context_refs, active_contexts);
                    let mut requires_contexts = omega_core::arena::HandleSpan::empty();
                    append_flow_contexts_for_points(
                        semantic,
                        &mut semantic_context_refs,
                        &mut requires_contexts,
                        &[ProgramPoint::CallRequires {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index: borrow_call.statement_index,
                            call_ordinal: borrow_call.call_ordinal,
                        }],
                    );
                    let mutated_places = call_mutated_places(
                        program,
                        machine.symbol,
                        state.symbol,
                        borrow,
                        borrow_call,
                        &mut state_mutation_summary_cache,
                    );
                    let post_call_contexts =
                        if call_may_mutate_contract_state(program, borrow, borrow_call) {
                            if mutated_places.is_empty() {
                                omega_core::arena::HandleSpan::empty()
                            } else {
                                filter_contexts_after_place_mutations(
                                    program,
                                    semantic,
                                    &mut domain_dependency_cache,
                                    &mut semantic_context_refs,
                                    active_contexts,
                                    &mutated_places,
                                )
                            }
                        } else {
                            clone_flow_contexts(&mut semantic_context_refs, active_contexts)
                        };
                    let mut exit_contexts =
                        clone_flow_contexts(&mut semantic_context_refs, post_call_contexts);
                    append_flow_contexts_for_points(
                        semantic,
                        &mut semantic_context_refs,
                        &mut exit_contexts,
                        &[ProgramPoint::CallEnsures {
                            machine_symbol: machine.symbol,
                            state_symbol: state.symbol,
                            statement_index: borrow_call.statement_index,
                            call_ordinal: borrow_call.call_ordinal,
                        }],
                    );
                    active_contexts =
                        clone_flow_contexts(&mut semantic_context_refs, exit_contexts);

                    calls.append_to_span(
                        &mut state_calls,
                        FlowCallFact {
                            statement_index: borrow_call.statement_index,
                            call_ordinal: borrow_call.call_ordinal,
                            receiver_symbol: borrow_call.receiver_symbol,
                            target_symbol: borrow_call.target_symbol,
                            has_receiver: borrow_call.has_receiver,
                            accesses: borrow_call.accesses,
                            entry_semantic_contexts: entry_contexts,
                            requires_contexts,
                            exit_semantic_contexts: exit_contexts,
                            requires: contract_call
                                .map(|call| call.requires)
                                .unwrap_or_else(HandleSpan::empty),
                            ensures: contract_call
                                .map(|call| call.ensures)
                                .unwrap_or_else(HandleSpan::empty),
                            direct_effects: effect_call
                                .map(|call| call.direct)
                                .unwrap_or_else(omega_effects::EffectSet::empty),
                            transitive_effects: effect_call
                                .map(|call| call.transitive)
                                .unwrap_or_else(omega_effects::EffectSet::empty),
                        },
                    );
                }

                if let Some(place) =
                    statement_mutated_place(program, machine, statement)
                {
                    active_contexts = filter_contexts_after_place_mutations(
                        program,
                        semantic,
                        &mut domain_dependency_cache,
                        &mut semantic_context_refs,
                        active_contexts,
                        &[place],
                    );
                }
            }

            let mut state_exits = omega_core::arena::HandleSpan::empty();
            for contract_exit in proof.contract_exits.iter().filter_map(|(_, exit)| {
                (exit.machine_symbol == machine.symbol && exit.state_symbol == state.symbol)
                    .then_some(exit)
            }) {
                let entry_exit_contexts =
                    clone_flow_contexts(&mut semantic_context_refs, active_contexts);
                let mut ensures_contexts = omega_core::arena::HandleSpan::empty();
                append_flow_contexts_for_points(
                    semantic,
                    &mut semantic_context_refs,
                    &mut ensures_contexts,
                    &[ProgramPoint::Exit {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: contract_exit.statement_index,
                    }],
                );

                exits.append_to_span(
                    &mut state_exits,
                    FlowExitFact {
                        machine_symbol: machine.symbol,
                        state_symbol: state.symbol,
                        statement_index: contract_exit.statement_index,
                        entry_semantic_contexts: entry_exit_contexts,
                        ensures_contexts,
                        ensures: contract_exit.ensures,
                    },
                );
            }

            states.append(FlowStateFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: borrow_state.writable_roots,
                mutable_parameter_count: borrow_state.mutable_parameter_count,
                entry_semantic_contexts: state_contexts,
                calls: state_calls,
                exits: state_exits,
                direct_effects: state_effects
                    .map(|state_effects| state_effects.direct)
                    .unwrap_or_else(omega_effects::EffectSet::empty),
                transitive_effects: state_effects
                    .map(|state_effects| state_effects.transitive)
                    .unwrap_or_else(omega_effects::EffectSet::empty),
            });
        }
    }

    FlowFacts {
        semantic_context_refs,
        calls,
        exits,
        states,
    }
}

fn clone_flow_contexts(
    semantic_context_refs: &mut omega_core::arena::Arena<FlowSemanticContextRef>,
    source: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
) -> omega_core::arena::HandleSpan<FlowSemanticContextRef> {
    let mut cloned = omega_core::arena::HandleSpan::empty();
    let copied: Vec<_> = semantic_context_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();
    for context_ref in copied {
        semantic_context_refs.append_to_span(&mut cloned, context_ref);
    }
    cloned
}

fn filter_contexts_after_place_mutations(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &mut DomainDependencyCache,
    semantic_context_refs: &mut omega_core::arena::Arena<FlowSemanticContextRef>,
    source: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    mutated_places: &[CanonicalPlace],
) -> omega_core::arena::HandleSpan<FlowSemanticContextRef> {
    if mutated_places.is_empty() {
        return source;
    }

    let mut filtered = omega_core::arena::HandleSpan::empty();
    let mut removed_any = false;
    let copied: Vec<_> = semantic_context_refs
        .span_or_empty(source)
        .iter()
        .copied()
        .collect();
    for context_ref in copied {
        let context = semantic.contexts.get(context_ref.context);
        if context_survives_place_mutations(
            program,
            semantic,
            domain_dependencies,
            context,
            mutated_places,
        ) {
            semantic_context_refs.append_to_span(&mut filtered, context_ref);
        } else {
            removed_any = true;
        }
    }

    if removed_any { filtered } else { source }
}

fn context_survives_place_mutations(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &mut DomainDependencyCache,
    context: &omega_facts::FactContext,
    mutated_places: &[CanonicalPlace],
) -> bool {
    !semantic.context_view(context).facts().any(|fact| {
        let FactPlace::Place(place) = fact.place else {
            return false;
        };
        mutated_places
            .iter()
            .any(|mutated_place| {
                canonical_place_overlaps_fact_place(
                    program,
                    semantic,
                    domain_dependencies,
                    fact,
                    place,
                    mutated_place,
                )
            })
    })
}

#[derive(Debug, Clone, Default)]
struct DomainDependencyCache {
    by_domain: Vec<DomainDependencyCacheEntry>,
}

#[derive(Debug, Clone)]
struct DomainDependencyCacheEntry {
    domain_symbol: SymbolHandle,
    dependencies: Vec<Vec<omega_facts::PlaceSegment>>,
}

#[derive(Debug, Clone, Default)]
struct StateMutationSummaryCache {
    states: Vec<StateMutationSummary>,
}

#[derive(Debug, Clone)]
struct StateMutationSummary {
    state_symbol: SymbolHandle,
    writes: Vec<CanonicalPlace>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct CanonicalPlace {
    root: omega_facts::PlaceRoot,
    segments: Vec<omega_facts::PlaceSegment>,
}

fn canonical_place_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<CanonicalPlace> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => canonical_place_from_expression(program, *inner),
        ExpressionNode::Name(path) => {
            let root_symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            let segments = program
                .expression_table
                .name_path_member_symbols(path.member_symbols)
                .iter()
                .skip(1)
                .copied()
                .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                .collect();
            Some(CanonicalPlace {
                root: omega_facts::PlaceRoot::Symbol(root_symbol),
                segments,
            })
        }
        ExpressionNode::Member(member) => {
            let mut place = canonical_place_from_expression(program, member.receiver)?;
            place.segments.push(omega_facts::PlaceSegment::Field {
                symbol: effective_member_symbol(program, member.receiver, member),
            });
            Some(place)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut place = canonical_place_from_expression(program, indexed.collection)?;
            place.segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(place)
        }
        _ => Some(CanonicalPlace {
            root: omega_facts::PlaceRoot::Expression(expression),
            segments: Vec::new(),
        }),
    }
}

fn canonical_place_from_symbol(symbol: SymbolHandle) -> Option<CanonicalPlace> {
    symbol.is_valid().then_some(CanonicalPlace {
        root: omega_facts::PlaceRoot::Symbol(symbol),
        segments: Vec::new(),
    })
}

fn effective_member_symbol(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member: &omega_typed_trees::expression::TableMemberExpression,
) -> SymbolHandle {
    if let Some(symbol) =
        resolve_member_symbol_from_receiver(program, receiver, member.member.as_str())
    {
        return symbol;
    }

    if member.member_symbol.is_valid() {
        return member.member_symbol;
    }

    SymbolHandle::invalid()
}

fn resolve_member_symbol_from_receiver(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = expression_type_symbol(program, receiver)?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.name.as_str() == member_name {
                return Some(contained.symbol);
            }
        }
    }

    None
}

fn expression_type_symbol(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
) -> Option<SymbolHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => expression_type_symbol(program, *inner),
        ExpressionNode::Name(path) => {
            let symbol = first_valid_name_path_symbol(path, &program.expression_table)?;
            symbol_type_symbol(program, symbol)
        }
        ExpressionNode::Member(member) => {
            let symbol = effective_member_symbol(program, member.receiver, member);
            symbol_type_symbol(program, symbol)
        }
        _ => None,
    }
}

fn symbol_type_symbol(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    if !symbol.is_valid() {
        return None;
    }

    for machine in program.machines() {
        if machine.symbol == symbol {
            if let Some(attached_data) = machine.attached_data.as_deref() {
                if let Some(data) = program
                    .data_definitions()
                    .iter()
                    .find(|definition| definition.name.as_str() == attached_data)
                {
                    return Some(data.symbol);
                }
            }
        }
        for state in program.machine_states(machine) {
            for parameter in program.state_parameters(state) {
                if parameter.symbol == symbol {
                    return Some(machine_symbol_from_type_reference_handle(
                        program,
                        parameter.type_reference,
                    ));
                }
            }
        }
        for owned in program.machine_owned_data(machine) {
            if owned.symbol == symbol {
                return Some(machine_symbol_from_type_reference_handle(
                    program,
                    owned.type_reference,
                ));
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.symbol == symbol {
                return Some(contained.type_symbol);
            }
        }
    }

    for data in program.data_definitions() {
        for member in program.data_members(data) {
            if let omega_typed_trees::data::DataMember::Field(field) = member
                && field.symbol == symbol
            {
                return Some(machine_symbol_from_type_reference_handle(
                    program,
                    field.type_reference,
                ));
            }
        }
    }

    None
}

fn canonical_place_segments_equal(
    left: omega_facts::PlaceSegment,
    right: omega_facts::PlaceSegment,
) -> bool {
    match (left, right) {
        (
            omega_facts::PlaceSegment::Field { symbol: left_symbol },
            omega_facts::PlaceSegment::Field {
                symbol: right_symbol,
            },
        ) => left_symbol == right_symbol,
        (
            omega_facts::PlaceSegment::Index {
                expression: left_expression,
            },
            omega_facts::PlaceSegment::Index {
                expression: right_expression,
            },
        ) => left_expression == right_expression,
        _ => false,
    }
}

fn canonical_place_overlaps_segments(
    left: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = left.len().min(right.len());
    left.iter()
        .take(shared_len)
        .zip(right.iter().take(shared_len))
        .all(|(left_segment, right_segment)| {
            canonical_place_segments_equal(*left_segment, *right_segment)
        })
}

fn canonical_place_overlaps_joined_segments(
    prefix: &[omega_facts::PlaceSegment],
    suffix: &[omega_facts::PlaceSegment],
    right: &[omega_facts::PlaceSegment],
) -> bool {
    let shared_len = prefix
        .len()
        .saturating_add(suffix.len())
        .min(right.len());

    (0..shared_len).all(|index| {
        let left_segment = if index < prefix.len() {
            prefix[index]
        } else {
            suffix[index - prefix.len()]
        };
        canonical_place_segments_equal(left_segment, right[index])
    })
}

fn canonical_place_overlaps_fact_place(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &mut DomainDependencyCache,
    fact: &Fact,
    fact_place: omega_facts::PlaceHandle,
    mutated_place: &CanonicalPlace,
) -> bool {
    let place = semantic.places.get(fact_place);
    let fact_segments = semantic.place_segments.span_or_empty(place.segments);
    if let Some(overlaps) = domain_membership_alias_overlaps(
        program,
        semantic,
        domain_dependencies,
        fact,
        place,
        fact_segments,
        mutated_place,
    ) {
        return overlaps;
    }

    if place.root != mutated_place.root {
        return false;
    }

    canonical_place_overlaps_segments(fact_segments, &mutated_place.segments)
}

fn domain_membership_alias_overlaps(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &mut DomainDependencyCache,
    fact: &Fact,
    fact_place: &omega_facts::Place,
    fact_segments: &[omega_facts::PlaceSegment],
    mutated_place: &CanonicalPlace,
) -> Option<bool> {
    let domain_symbol = match fact.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return None,
    };

    if fact_place.root != mutated_place.root {
        return Some(false);
    }

    let dependencies =
        domain_dependency_segments(program, semantic, domain_dependencies, domain_symbol);
    if dependencies.is_empty() {
        return Some(canonical_place_overlaps_segments(
            fact_segments,
            &mutated_place.segments,
        ));
    }

    Some(dependencies.iter().any(|dependency_segments| {
        canonical_place_overlaps_joined_segments(
            fact_segments,
            dependency_segments,
            &mutated_place.segments,
        )
    }))
}

fn domain_dependency_segments<'cache>(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &'cache mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
) -> &'cache [Vec<omega_facts::PlaceSegment>] {
    if !cache.by_domain.iter().any(|entry| entry.domain_symbol == domain_symbol) {
        let mut visiting = BTreeSet::new();
        let dependencies = compute_domain_dependency_segments(
            program,
            semantic,
            cache,
            domain_symbol,
            &mut visiting,
        );
        cache.by_domain.push(DomainDependencyCacheEntry {
            domain_symbol,
            dependencies,
        });
    }

    cache
        .by_domain
        .iter()
        .find(|entry| entry.domain_symbol == domain_symbol)
        .map(|entry| entry.dependencies.as_slice())
        .unwrap_or(&[])
}

fn compute_domain_dependency_segments(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    cache: &mut DomainDependencyCache,
    domain_symbol: SymbolHandle,
    visiting: &mut BTreeSet<u32>,
) -> Vec<Vec<omega_facts::PlaceSegment>> {
    if let Some(cached) = cache
        .by_domain
        .iter()
        .find(|entry| entry.domain_symbol == domain_symbol)
    {
        return cached.dependencies.clone();
    }
    let domain_key = domain_symbol.arena_index();
    if !visiting.insert(domain_key) {
        return vec![Vec::new()];
    }

    let mut dependencies = Vec::new();
    let self_type_symbol = program
        .domain_definitions()
        .iter()
        .find(|domain| domain.symbol == domain_symbol)
        .map(|domain| machine_symbol_from_type_reference_handle(program, domain.target_type))
        .filter(|symbol| symbol.is_valid());
    for fact in semantic.facts_for_symbol(domain_symbol) {
        match fact.payload {
            FactPayload::BooleanExpression(expression) => {
                collect_dependency_paths_from_expression(
                    program,
                    expression,
                    self_type_symbol,
                    &mut dependencies,
                );
            }
            FactPayload::DomainMembership {
                domain_symbol: imported_domain,
                ..
            } => {
                let FactPlace::Place(place_handle) = fact.place else {
                    dependencies.push(Vec::new());
                    continue;
                };
                let place = semantic.places.get(place_handle);
                let base_segments: Vec<_> = semantic
                    .place_segments
                    .span_or_empty(place.segments)
                    .iter()
                    .copied()
                    .collect();
                let imported_dependencies = compute_domain_dependency_segments(
                    program,
                    semantic,
                    cache,
                    imported_domain,
                    visiting,
                );
                if imported_dependencies.is_empty() {
                    dependencies.push(base_segments);
                } else {
                    for imported in imported_dependencies {
                        let mut rebased = Vec::with_capacity(
                            base_segments.len().saturating_add(imported.len()),
                        );
                        rebased.extend(base_segments.iter().copied());
                        rebased.extend(imported);
                        dependencies.push(rebased);
                    }
                }
            }
            _ => {}
        }
    }

    visiting.remove(&domain_key);
    dedupe_dependency_segments(&mut dependencies);
    dependencies
}

fn collect_dependency_paths_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    self_type_symbol: Option<SymbolHandle>,
    dependencies: &mut Vec<Vec<omega_facts::PlaceSegment>>,
) {
    if !expression.is_valid() {
        return;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_dependency_paths_from_expression(
                    program,
                    *value,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_dependency_paths_from_expression(
                program,
                binary.left,
                self_type_symbol,
                dependencies,
            );
            collect_dependency_paths_from_expression(
                program,
                binary.right,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_dependency_paths_from_expression(
                    program,
                    call.receiver,
                    self_type_symbol,
                    dependencies,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_dependency_paths_from_expression(
                    program,
                    *argument,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Cast(cast) => {
            collect_dependency_paths_from_expression(
                program,
                cast.value,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Indexed(indexed) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    indexed.collection,
                    self_type_symbol,
                    dependencies,
                );
            }
            collect_dependency_paths_from_expression(
                program,
                indexed.index,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Member(member) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            } else {
                collect_dependency_paths_from_expression(
                    program,
                    member.receiver,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Mutable(inner) => {
            collect_dependency_paths_from_expression(
                program,
                *inner,
                self_type_symbol,
                dependencies,
            );
        }
        ExpressionNode::Name(_) => {
            if let Some(place) = canonical_place_from_expression(program, expression) {
                dependencies.push(place.segments);
            } else if let Some(segments) =
                relative_place_segments_from_expression(program, expression, self_type_symbol)
            {
                dependencies.push(segments);
            }
        }
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program.expression_table.struct_fields(struct_literal.fields) {
                collect_dependency_paths_from_expression(
                    program,
                    field.value,
                    self_type_symbol,
                    dependencies,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

fn relative_place_segments_from_expression(
    program: &omega_typed_trees::TypedTrees,
    expression: ExpressionHandle,
    self_type_symbol: Option<SymbolHandle>,
) -> Option<Vec<omega_facts::PlaceSegment>> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            relative_place_segments_from_expression(program, *inner, self_type_symbol)
        }
        ExpressionNode::Name(path) => {
            let members = program.expression_table.name_path_members(path.members);
            let head = members.first()?.as_str();
            if head != "self" {
                return None;
            }

            Some(Vec::new())
        }
        ExpressionNode::Member(member) => {
            let mut segments = relative_place_segments_from_expression(
                program,
                member.receiver,
                self_type_symbol,
            )?;
            let member_symbol = if let Some(symbol) =
                resolve_member_symbol_from_type(program, self_type_symbol, member.member.as_str())
            {
                symbol
            } else {
                effective_member_symbol(program, member.receiver, member)
            };
            segments.push(omega_facts::PlaceSegment::Field {
                symbol: member_symbol,
            });
            Some(segments)
        }
        ExpressionNode::Indexed(indexed) => {
            let mut segments = relative_place_segments_from_expression(
                program,
                indexed.collection,
                self_type_symbol,
            )?;
            segments.push(omega_facts::PlaceSegment::Index {
                expression: indexed.index,
            });
            Some(segments)
        }
        _ => None,
    }
}

fn resolve_member_symbol_from_type(
    program: &omega_typed_trees::TypedTrees,
    type_symbol: Option<SymbolHandle>,
    member_name: &str,
) -> Option<SymbolHandle> {
    let type_symbol = type_symbol?;

    if let Some(data) = program
        .data_definitions()
        .iter()
        .find(|definition| definition.symbol == type_symbol)
    {
        for member in program.data_members(data) {
            match member {
                omega_typed_trees::data::DataMember::Field(field)
                    if field.name.as_str() == member_name =>
                {
                    return Some(field.symbol);
                }
                omega_typed_trees::data::DataMember::Variant(variant)
                    if variant.name.as_str() == member_name =>
                {
                    return Some(variant.symbol);
                }
                _ => {}
            }
        }
    }

    if let Some(machine) = machine_by_symbol(program, type_symbol) {
        for owned in program.machine_owned_data(machine) {
            if owned.name.as_str() == member_name {
                return Some(owned.symbol);
            }
        }
        for contained in program.machine_contained_objects(machine) {
            if contained.name.as_str() == member_name {
                return Some(contained.symbol);
            }
        }
    }

    None
}

fn dedupe_dependency_segments(dependencies: &mut Vec<Vec<omega_facts::PlaceSegment>>) {
    let mut unique: Vec<Vec<omega_facts::PlaceSegment>> = Vec::with_capacity(dependencies.len());
    for dependency in dependencies.drain(..) {
        if !unique.iter().any(|existing| {
            existing.len() == dependency.len()
                && existing
                    .iter()
                    .zip(dependency.iter())
                    .all(|(left, right)| canonical_place_segments_equal(*left, *right))
        }) {
            unique.push(dependency);
        }
    }
    *dependencies = unique;
}

fn call_mutated_places(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
    state_mutation_summaries: &mut StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    let summarized_places = instantiate_call_mutation_summary_places(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call,
        state_mutation_summaries,
    );
    if !summarized_places.is_empty() {
        return summarized_places;
    }

    let mut places = Vec::new();
    for access in borrow.argument_accesses.span_or_empty(borrow_call.accesses) {
        if access.kind == BorrowAccessKind::Mutable
            && let Some(place) = canonical_place_from_symbol(access.root_symbol)
            && !places.contains(&place)
        {
            places.push(place);
        }
    }

    if let Some(call_site) = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    ) && let Some(target_state) = find_state(program, borrow_call.target_symbol)
    {
        let mut argument_index = 0usize;
        for parameter in program.state_parameters(target_state) {
            if parameter.is_self {
                continue;
            }

            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);

            if !parameter.is_mutable {
                continue;
            }

            if let Some(argument) = argument
                && let Some(place) = canonical_place_from_expression(program, argument)
                && !places.contains(&place)
            {
                places.push(place);
            }
        }
    }

    if borrow_call.has_receiver
        && call_receiver_is_mutable(program, borrow, borrow_call)
        && let Some(place) = call_receiver_mutated_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
        )
        && !places.contains(&place)
    {
        places.push(place);
    }

    places
}

fn call_receiver_is_mutable(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
) -> bool {
    let Some((target_machine_symbol, target_state_symbol)) =
        contract_target_from_state_symbol(program, borrow_call.target_symbol)
    else {
        return false;
    };
    let Some(state) = find_state_in_machine(program, target_machine_symbol, target_state_symbol)
    else {
        return false;
    };
    program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_self && parameter.is_mutable)
        || borrow_call.accesses.is_empty()
            && borrow.states.iter().any(|(_, flow_state)| {
                flow_state.machine_symbol == target_machine_symbol
                    && flow_state.state_symbol == target_state_symbol
                    && flow_state.mutable_parameter_count > 0
            })
}

fn call_may_mutate_contract_state(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    borrow_call: &BorrowCallFact,
) -> bool {
    let Some((target_machine_symbol, target_state_symbol)) =
        contract_target_from_state_symbol(program, borrow_call.target_symbol)
    else {
        return false;
    };
    let Some(state) = find_state_in_machine(program, target_machine_symbol, target_state_symbol)
    else {
        return false;
    };
    let signature_mutability = program
        .state_parameters(state)
        .iter()
        .any(|parameter| parameter.is_mutable);
    let borrow_mutability = borrow.states.iter().any(|(_, flow_state)| {
        flow_state.machine_symbol == target_machine_symbol
            && flow_state.state_symbol == target_state_symbol
            && flow_state.mutable_parameter_count > 0
    });

    signature_mutability
        || borrow_mutability
        || call_receiver_is_mutable(program, borrow, borrow_call)
}

fn call_receiver_mutated_place(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
) -> Option<CanonicalPlace> {
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    match call_site {
        CallSite::Statement(statement) => {
            if let Some(path) = statement_call_receiver_path(program, statement) {
                return Some(CanonicalPlace {
                    root: omega_facts::PlaceRoot::Symbol(path.head_symbol()),
                    segments: path
                        .member_symbols()
                        .iter()
                        .skip(1)
                        .copied()
                        .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                        .collect(),
                });
            }
            canonical_place_from_symbol(statement.receiver_symbol)
        }
        CallSite::Expression(call) => {
            if call.receiver.is_valid() {
                canonical_place_from_expression(program, call.receiver)
            } else {
                let caller_state =
                    find_state_in_machine(program, caller_machine_symbol, caller_state_symbol)?;
                let self_parameter = program
                    .state_parameters(caller_state)
                    .iter()
                    .find(|parameter| parameter.is_self)?;
                canonical_place_from_symbol(self_parameter.symbol)
            }
        }
    }
}

fn instantiate_call_mutation_summary_places(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    cache: &mut StateMutationSummaryCache,
) -> Vec<CanonicalPlace> {
    let Some(target_state) = find_state(program, borrow_call.target_symbol) else {
        return Vec::new();
    };
    let summary_places = state_mutation_summary_places(program, cache, target_state);
    if summary_places.is_empty() {
        return Vec::new();
    }

    let mut instantiated = Vec::new();
    for summary_place in summary_places {
        if let Some(place) = instantiate_call_relative_place(
            program,
            caller_machine_symbol,
            caller_state_symbol,
            borrow_call,
            summary_place,
        ) && !instantiated.contains(&place)
        {
            instantiated.push(place);
        }
    }

    instantiated
}

fn state_mutation_summary_places<'cache>(
    program: &omega_typed_trees::TypedTrees,
    cache: &'cache mut StateMutationSummaryCache,
    state: &omega_typed_trees::state::State,
) -> &'cache [CanonicalPlace] {
    if !cache.states.iter().any(|entry| entry.state_symbol == state.symbol) {
        let writes = collect_state_mutation_summary_places(program, state);
        cache.states.push(StateMutationSummary {
            state_symbol: state.symbol,
            writes,
        });
    }

    cache
        .states
        .iter()
        .find(|entry| entry.state_symbol == state.symbol)
        .map(|entry| entry.writes.as_slice())
        .unwrap_or(&[])
}

fn collect_state_mutation_summary_places(
    program: &omega_typed_trees::TypedTrees,
    state: &omega_typed_trees::state::State,
) -> Vec<CanonicalPlace> {
    let parameter_symbols: Vec<_> = program
        .state_parameters(state)
        .iter()
        .map(|parameter| parameter.symbol)
        .collect();
    let mut writes = Vec::new();

    for statement in program.statement_table.statements(state.statement_nodes) {
        let StatementNode::Assignment(assignment) = statement else {
            continue;
        };
        let Some(place) = canonical_place_from_expression(program, assignment.target) else {
            continue;
        };
        let omega_facts::PlaceRoot::Symbol(root_symbol) = place.root else {
            continue;
        };
        if parameter_symbols.contains(&root_symbol) && !writes.contains(&place) {
            writes.push(place);
        }
    }

    writes
}

fn instantiate_call_relative_place(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    borrow_call: &BorrowCallFact,
    relative_place: &CanonicalPlace,
) -> Option<CanonicalPlace> {
    let omega_facts::PlaceRoot::Symbol(parameter_symbol) = relative_place.root else {
        return None;
    };
    let call_site = find_call_site(
        program,
        caller_machine_symbol,
        caller_state_symbol,
        borrow_call.statement_index,
        borrow_call.call_ordinal,
    )?;
    let target_state = find_state(program, borrow_call.target_symbol)?;
    let mut argument_index = 0usize;

    for parameter in program.state_parameters(target_state) {
        let base_place = if parameter.is_self {
            if parameter.symbol != parameter_symbol {
                continue;
            }
            canonical_receiver_place_for_call_site(
                program,
                caller_machine_symbol,
                caller_state_symbol,
                &call_site,
            )
        } else {
            let argument = call_site_argument_expressions(program, &call_site)
                .get(argument_index)
                .copied();
            argument_index = argument_index.saturating_add(1);
            if parameter.symbol != parameter_symbol {
                continue;
            }
            argument.and_then(|expression| canonical_place_from_expression(program, expression))
        }?;

        let mut instantiated = base_place;
        instantiated
            .segments
            .extend(relative_place.segments.iter().copied());
        return Some(instantiated);
    }

    None
}

fn canonical_receiver_place_for_call_site(
    program: &omega_typed_trees::TypedTrees,
    caller_machine_symbol: SymbolHandle,
    caller_state_symbol: SymbolHandle,
    call_site: &CallSite<'_>,
) -> Option<CanonicalPlace> {
    match call_site {
        CallSite::Statement(statement) => {
            if let Some(path) = statement_call_receiver_path(program, statement) {
                return Some(CanonicalPlace {
                    root: omega_facts::PlaceRoot::Symbol(path.head_symbol()),
                    segments: path
                        .member_symbols()
                        .iter()
                        .skip(1)
                        .copied()
                        .map(|symbol| omega_facts::PlaceSegment::Field { symbol })
                        .collect(),
                });
            }
            canonical_place_from_symbol(statement.receiver_symbol)
        }
        CallSite::Expression(call) => {
            if call.receiver.is_valid() {
                return canonical_place_from_expression(program, call.receiver);
            }

            let caller_state =
                find_state_in_machine(program, caller_machine_symbol, caller_state_symbol)?;
            let self_parameter = program
                .state_parameters(caller_state)
                .iter()
                .find(|parameter| parameter.is_self)?;
            canonical_place_from_symbol(self_parameter.symbol)
        }
    }
}

fn append_flow_contexts_for_points(
    semantic: &FactPlan,
    semantic_context_refs: &mut omega_core::arena::Arena<FlowSemanticContextRef>,
    refs: &mut omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    points: &[ProgramPoint],
) {
    for point in points {
        for context in semantic.context_handles_at_point(*point) {
            semantic_context_refs.append_to_span(refs, FlowSemanticContextRef { context });
        }
    }
}

fn statement_mutated_place(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    statement: &StatementNode,
) -> Option<CanonicalPlace> {
    match statement {
        StatementNode::Assignment(assignment) => {
            canonical_place_from_expression(program, assignment.target).or_else(|| {
                expression_root_symbol(assignment.target, &program.expression_table, machine.symbol)
                    .and_then(canonical_place_from_symbol)
            })
        }
        _ => None,
    }
}

fn borrow_state_fact(
    borrow: &BorrowFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
) -> Option<&StateBorrowFact> {
    borrow.states.iter().find_map(|(_, state)| {
        (state.machine_symbol == machine_symbol && state.state_symbol == state_symbol)
            .then_some(state)
    })
}

fn proof_contract_call(
    proof: &ProofFacts,
    machine_symbol: SymbolHandle,
    state_symbol: SymbolHandle,
    statement_index: usize,
    call_ordinal: usize,
) -> Option<&ContractCallFact> {
    proof.contract_calls.iter().find_map(|(_, call)| {
        (call.caller_machine_symbol == machine_symbol
            && call.caller_state_symbol == state_symbol
            && call.statement_index == statement_index
            && call.call_ordinal == call_ordinal)
            .then_some(call)
    })
}

fn effects_machine(
    effects: &omega_effects::EffectPlan,
    machine_symbol: SymbolHandle,
) -> Option<&omega_effects::MachineEffects> {
    effects
        .machines()
        .iter()
        .find(|machine| machine.symbol == machine_symbol)
}

fn effects_state<'effects>(
    effects: &'effects omega_effects::EffectPlan,
    machine_effects: Option<&'effects omega_effects::MachineEffects>,
    state_symbol: SymbolHandle,
) -> Option<&'effects omega_effects::StateEffects> {
    machine_effects.and_then(|machine_effects| {
        effects
            .states
            .span_or_empty(machine_effects.states)
            .iter()
            .find(|state| state.symbol == state_symbol)
    })
}

fn effects_call<'effects>(
    effects: &'effects omega_effects::EffectPlan,
    state_effects: Option<&'effects omega_effects::StateEffects>,
    borrow_call: &BorrowCallFact,
) -> Option<&'effects omega_effects::CallEffects> {
    state_effects.and_then(|state_effects| {
        effects
            .calls
            .span_or_empty(state_effects.calls)
            .iter()
            .find(|call| {
                call.statement_index == borrow_call.statement_index
                    && call.call_ordinal == borrow_call.call_ordinal
                    && call.target_state_symbol == borrow_call.target_symbol
            })
    })
}

fn build_borrow_facts(program: &omega_typed_trees::TypedTrees) -> BorrowFacts {
    let mut writable_roots =
        omega_core::arena::Arena::with_capacity(estimated_borrow_root_capacity(program));
    let mut argument_accesses =
        omega_core::arena::Arena::with_capacity(program.expression_table.expression_count());
    let mut calls =
        omega_core::arena::Arena::with_capacity(program.statement_table.statement_count());
    let mut states = omega_core::arena::Arena::with_capacity(machine_state_count(program));

    for machine in program.machines() {
        for state in program.machine_states(machine) {
            let mut writable_roots_span = omega_core::arena::HandleSpan::empty();
            for field in attached_data_fields(program, machine) {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: field.symbol,
                        kind: BorrowRootKind::OwnedData,
                    },
                );
            }

            for owned in program.machine_owned_data(machine) {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: owned.symbol,
                        kind: BorrowRootKind::OwnedData,
                    },
                );
            }

            for statement in program.statement_table.statements(state.statement_nodes) {
                let StatementNode::LocalData(local_data) = statement else {
                    continue;
                };
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: local_data.symbol,
                        kind: BorrowRootKind::LocalData,
                    },
                );
            }

            for parameter in program
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.is_mutable)
            {
                writable_roots.append_to_span(
                    &mut writable_roots_span,
                    BorrowWritableRootFact {
                        symbol: parameter.symbol,
                        kind: BorrowRootKind::MutableParameter,
                    },
                );
            }

            let mut calls_span = omega_core::arena::HandleSpan::empty();
            for (statement_index, statement) in program
                .statement_table
                .statements(state.statement_nodes)
                .iter()
                .enumerate()
            {
                let mut call_ordinal = 0usize;
                collect_statement_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    statement,
                    &mut call_ordinal,
                    &mut argument_accesses,
                    &mut calls,
                    &mut calls_span,
                );
            }

            let mutable_parameter_count = program
                .state_parameters(state)
                .iter()
                .filter(|parameter| parameter.is_mutable)
                .count();

            states.append(StateBorrowFact {
                machine_symbol: machine.symbol,
                state_symbol: state.symbol,
                writable_roots: writable_roots_span,
                mutable_parameter_count,
                calls: calls_span,
            });
        }
    }

    BorrowFacts {
        writable_roots,
        argument_accesses,
        calls,
        states,
    }
}

fn machine_state_count(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| program.machine_states(machine).len())
        .sum()
}

fn attached_data_fields<'program>(
    program: &'program omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
) -> impl Iterator<Item = &'program omega_typed_trees::data::DataField> {
    machine
        .attached_data
        .as_ref()
        .and_then(|attached_data| {
            program
                .data_definitions()
                .iter()
                .find(|definition| definition.name == *attached_data)
        })
        .into_iter()
        .flat_map(|definition| program.data_members(definition).iter())
        .filter_map(|member| match member {
            omega_typed_trees::data::DataMember::Field(field) => Some(field),
            omega_typed_trees::data::DataMember::Variant(_) => None,
        })
}

fn estimated_borrow_root_capacity(program: &omega_typed_trees::TypedTrees) -> usize {
    program
        .machines()
        .iter()
        .map(|machine| {
            program
                .machine_states(machine)
                .iter()
                .map(|state| {
                    let local_data_count = program
                        .statement_table
                        .statements(state.statement_nodes)
                        .iter()
                        .filter(|statement| matches!(statement, StatementNode::LocalData(_)))
                        .count();
                    let mutable_parameter_count = program
                        .state_parameters(state)
                        .iter()
                        .filter(|parameter| parameter.is_mutable)
                        .count();

                    program.machine_owned_data(machine).len()
                        + attached_data_fields(program, machine).count()
                        + local_data_count
                        + mutable_parameter_count
                })
                .sum::<usize>()
        })
        .sum()
}

fn collect_statement_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    statement: &StatementNode,
    call_ordinal: &mut usize,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match statement {
        StatementNode::Assignment(assignment) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            assignment.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::Call(call) => {
            if statement_call_can_dispatch_to_machine(program, machine, state, call) {
                let receiver_path = statement_call_receiver_path(program, call);
                append_borrow_call(
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    call.receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_ref(),
                    collect_call_argument_accesses(
                        argument_accesses,
                        &program.expression_table,
                        program.statement_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            for argument in program.statement_table.expression_handles(call.arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Expression(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        StatementNode::LocalData(local_data) => {
            if local_data.initial_value.is_valid() {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    local_data.initial_value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        StatementNode::Transition(transition) => {
            if let TransitionGuardNode::When(expression) = transition.guard {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    expression,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }

            collect_transition_target_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                transition.target,
                argument_accesses,
                calls,
                state_calls,
            );

            if transition.continuation.is_valid() {
                collect_transition_target_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    transition.continuation,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
    }
}

fn collect_transition_target_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &mut usize,
    target: omega_typed_trees::statement::TransitionTargetHandle,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.statement_table.transition_target(target) {
        TransitionTargetNode::Named { arguments, .. } => {
            for argument in program.statement_table.expression_handles(*arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        TransitionTargetNode::Value(expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        TransitionTargetNode::SelfTarget | TransitionTargetNode::Terminal => {}
    }
}

fn append_borrow_call(
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
    statement_index: usize,
    call_ordinal: usize,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver_path: Option<&NamePath>,
    accesses: omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
) {
    calls.append_to_span(
        state_calls,
        BorrowCallFact {
            statement_index,
            call_ordinal,
            receiver_symbol,
            target_symbol,
            has_receiver: receiver_path.is_some(),
            accesses,
        },
    );
}

fn collect_expression_borrow_calls(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    statement_index: usize,
    call_ordinal: &mut usize,
    expression: ExpressionHandle,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    calls: &mut omega_core::arena::Arena<BorrowCallFact>,
    state_calls: &mut omega_core::arena::HandleSpan<BorrowCallFact>,
) {
    match program.expression_table.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in program.expression_table.expression_handles(*values) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                binary.left,
                argument_accesses,
                calls,
                state_calls,
            );
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                binary.right,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        ExpressionNode::Call(call) => {
            let (receiver_symbol, receiver_path) = call_receiver_parts(program, call.receiver);
            let is_machine_call = resolve_state_call_target(
                program,
                machine,
                state,
                receiver_symbol,
                call.target_symbol,
                receiver_path.as_deref(),
                &call.target,
            )
            .is_valid()
                || receiver_can_dispatch_to_machine(
                    program,
                    machine,
                    state,
                    receiver_symbol,
                    receiver_path.as_deref(),
                );

            if is_machine_call {
                append_borrow_call(
                    calls,
                    state_calls,
                    statement_index,
                    *call_ordinal,
                    receiver_symbol,
                    call.target_symbol,
                    receiver_path.as_ref(),
                    collect_call_argument_accesses(
                        argument_accesses,
                        &program.expression_table,
                        program.expression_table.expression_handles(call.arguments),
                        machine.symbol,
                    ),
                );
                *call_ordinal += 1;
            }

            if call.receiver.is_valid() {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    call.receiver,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
            for argument in program.expression_table.expression_handles(call.arguments) {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    *argument,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            cast.value,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::Indexed(indexed) => {
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                indexed.collection,
                argument_accesses,
                calls,
                state_calls,
            );
            collect_expression_borrow_calls(
                program,
                machine,
                state,
                statement_index,
                call_ordinal,
                indexed.index,
                argument_accesses,
                calls,
                state_calls,
            );
        }
        ExpressionNode::Member(member) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            member.receiver,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::Mutable(inner_expression) => collect_expression_borrow_calls(
            program,
            machine,
            state,
            statement_index,
            call_ordinal,
            *inner_expression,
            argument_accesses,
            calls,
            state_calls,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in program
                .expression_table
                .struct_fields(struct_literal.fields)
            {
                collect_expression_borrow_calls(
                    program,
                    machine,
                    state,
                    statement_index,
                    call_ordinal,
                    field.value,
                    argument_accesses,
                    calls,
                    state_calls,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::Name(_)
        | ExpressionNode::String(_) => {}
    }
}

fn statement_call_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    call: &TableCall,
) -> bool {
    resolve_state_call_target(
        program,
        machine,
        state,
        call.receiver_symbol,
        call.target_symbol,
        statement_call_receiver_members(program, call),
        &call.target,
    )
    .is_valid()
        || receiver_can_dispatch_to_machine(
            program,
            machine,
            state,
            call.receiver_symbol,
            statement_call_receiver_members(program, call),
        )
}

fn statement_call_receiver_members<'a>(
    program: &'a omega_typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<&'a [ProgramName]> {
    (!call.receiver.is_empty()).then(|| program.statement_table.name_path_members(call.receiver))
}

fn statement_call_receiver_path(
    program: &omega_typed_trees::TypedTrees,
    call: &TableCall,
) -> Option<NamePath> {
    let members = statement_call_receiver_members(program, call)?;

    Some(NamePath::resolved_from_iter(
        members.iter().cloned(),
        call.receiver_symbol,
        call.receiver_symbol,
    ))
}

fn call_receiver_parts(
    program: &omega_typed_trees::TypedTrees,
    receiver: ExpressionHandle,
) -> (
    SymbolHandle,
    Option<omega_checked_trees::expression::NamePath>,
) {
    if !receiver.is_valid() {
        return (SymbolHandle::invalid(), None);
    }

    match program.expression_table.expression(receiver) {
        ExpressionNode::Mutable(inner) => call_receiver_parts(program, *inner),
        ExpressionNode::Name(path) => (
            path.symbol,
            Some(NamePath::resolved_from_iter(
                program
                    .expression_table
                    .name_path_members(path.members)
                    .iter()
                    .cloned(),
                path.head_symbol,
                path.symbol,
            )),
        ),
        ExpressionNode::Member(member) => {
            let (_, path) = call_receiver_parts(program, member.receiver);
            let mut path = path.unwrap_or_default();
            path.push_resolved(member.member.clone(), member.member_symbol);
            (member.member_symbol, Some(path))
        }
        _ => (SymbolHandle::invalid(), None),
    }
}

fn resolve_state_call_target(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    target_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
    _target_state: &ProgramName,
) -> SymbolHandle {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return resolve_state_symbol_in_machine(program, machine, target_symbol);
    }

    if !receiver_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    if let Some(contained) = program
        .machine_contained_objects(machine)
        .iter()
        .find(|contained| contained.symbol == receiver_symbol)
    {
        let Some(target_machine) = machine_by_symbol(program, contained.type_symbol) else {
            return SymbolHandle::invalid();
        };
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if let Some(target_machine) = machine_by_symbol(program, receiver_symbol) {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .unwrap_or_else(SymbolHandle::invalid);
    if type_symbol.is_valid()
        && let Some(target_machine) = machine_by_symbol(program, type_symbol)
    {
        return resolve_state_symbol_in_machine(program, target_machine, target_symbol);
    }

    if target_symbol.is_valid()
        && program
            .machines()
            .iter()
            .flat_map(|machine| program.machine_states(machine).iter())
            .any(|state| state.symbol == target_symbol)
    {
        return target_symbol;
    }

    SymbolHandle::invalid()
}

fn receiver_can_dispatch_to_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state: &omega_typed_trees::state::State,
    receiver_symbol: SymbolHandle,
    receiver: Option<&[ProgramName]>,
) -> bool {
    if receiver.is_none() || receiver_symbol == machine.symbol {
        return true;
    }

    if !receiver_symbol.is_valid() {
        return false;
    }

    if program
        .machine_contained_objects(machine)
        .iter()
        .any(|contained| contained.symbol == receiver_symbol)
    {
        return true;
    }

    let type_symbol = program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.symbol == receiver_symbol)
        .map(|parameter| {
            machine_symbol_from_type_reference_handle(program, parameter.type_reference)
        })
        .unwrap_or_else(SymbolHandle::invalid);

    machine_by_symbol(program, receiver_symbol).is_some()
        || (type_symbol.is_valid() && machine_by_symbol(program, type_symbol).is_some())
}

fn resolve_state_symbol_in_machine(
    program: &omega_typed_trees::TypedTrees,
    machine: &omega_typed_trees::machine::Machine,
    state_symbol: SymbolHandle,
) -> SymbolHandle {
    if !state_symbol.is_valid() {
        return SymbolHandle::invalid();
    }

    program
        .machine_states(machine)
        .iter()
        .find(|state| state.symbol == state_symbol)
        .map(|state| state.symbol)
        .unwrap_or_else(SymbolHandle::invalid)
}

fn machine_by_symbol(
    program: &omega_typed_trees::TypedTrees,
    symbol: SymbolHandle,
) -> Option<&omega_typed_trees::machine::Machine> {
    program
        .machines()
        .iter()
        .find(|machine| machine.symbol == symbol)
}

fn machine_symbol_from_type_reference_handle(
    program: &omega_typed_trees::TypedTrees,
    type_reference: omega_typed_trees::types::TypeReferenceHandle,
) -> SymbolHandle {
    match program.type_reference_table.type_reference(type_reference) {
        omega_typed_trees::types::TypeReferenceNode::Reference { referee, .. } => {
            machine_symbol_from_type_reference_handle(program, *referee)
        }
        omega_typed_trees::types::TypeReferenceNode::Constrained { base_type, .. } => {
            machine_symbol_from_type_reference_handle(program, *base_type)
        }
        omega_typed_trees::types::TypeReferenceNode::Generic { base_symbol, .. }
        | omega_typed_trees::types::TypeReferenceNode::Named {
            symbol: base_symbol,
            ..
        } => *base_symbol,
        omega_typed_trees::types::TypeReferenceNode::FixedArray { .. }
        | omega_typed_trees::types::TypeReferenceNode::Slice { .. }
        | omega_typed_trees::types::TypeReferenceNode::Unit => SymbolHandle::invalid(),
    }
}

fn collect_call_argument_accesses(
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    arguments: &[ExpressionHandle],
    machine_symbol: SymbolHandle,
) -> omega_core::arena::HandleSpan<BorrowArgumentAccessFact> {
    let mut accesses = omega_core::arena::HandleSpan::empty();

    for argument in arguments {
        collect_argument_accesses(
            *argument,
            expressions,
            argument_accesses,
            &mut accesses,
            machine_symbol,
        );
    }

    accesses
}

fn collect_argument_accesses(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match expressions.expression(expression) {
        ExpressionNode::Mutable(inner_expression) => {
            if let Some(root_symbol) =
                expression_root_symbol(*inner_expression, expressions, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Mutable,
                    },
                );
            }
        }
        _ => collect_read_accesses(
            expression,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
    }
}

fn collect_read_accesses(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    argument_accesses: &mut omega_core::arena::Arena<BorrowArgumentAccessFact>,
    accesses: &mut omega_core::arena::HandleSpan<BorrowArgumentAccessFact>,
    machine_symbol: SymbolHandle,
) {
    match expressions.expression(expression) {
        ExpressionNode::ArrayLiteral(values) => {
            for value in expressions.expression_handles(*values) {
                collect_read_accesses(
                    *value,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Binary(binary) => {
            collect_read_accesses(
                binary.left,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
            collect_read_accesses(
                binary.right,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Call(call) => {
            if call.receiver.is_valid() {
                collect_read_accesses(
                    call.receiver,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }

            for argument in expressions.expression_handles(call.arguments) {
                collect_read_accesses(
                    *argument,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Cast(cast) => collect_read_accesses(
            cast.value,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Indexed(indexed) => {
            if let Some(root_symbol) =
                expression_root_symbol(indexed.collection, expressions, machine_symbol)
            {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Read,
                    },
                );
            }

            collect_read_accesses(
                indexed.index,
                expressions,
                argument_accesses,
                accesses,
                machine_symbol,
            );
        }
        ExpressionNode::Member(member) => collect_read_accesses(
            member.receiver,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::Name(path) => {
            if let Some(root_symbol) = first_valid_name_path_symbol(path, expressions) {
                argument_accesses.append_to_span(
                    accesses,
                    BorrowArgumentAccessFact {
                        root_symbol,
                        kind: BorrowAccessKind::Read,
                    },
                );
            }
        }
        ExpressionNode::Mutable(inner_expression) => collect_read_accesses(
            *inner_expression,
            expressions,
            argument_accesses,
            accesses,
            machine_symbol,
        ),
        ExpressionNode::StructLiteral(struct_literal) => {
            for field in expressions.struct_fields(struct_literal.fields) {
                collect_read_accesses(
                    field.value,
                    expressions,
                    argument_accesses,
                    accesses,
                    machine_symbol,
                );
            }
        }
        ExpressionNode::Boolean(_)
        | ExpressionNode::Float(_)
        | ExpressionNode::Integer(_)
        | ExpressionNode::String(_) => {}
    }
}

fn expression_root_symbol(
    expression: ExpressionHandle,
    expressions: &omega_typed_trees::expression::ExpressionTable,
    machine_symbol: SymbolHandle,
) -> Option<SymbolHandle> {
    match expressions.expression(expression) {
        ExpressionNode::Indexed(indexed) => {
            expression_root_symbol(indexed.collection, expressions, machine_symbol)
        }
        ExpressionNode::Mutable(inner) => {
            expression_root_symbol(*inner, expressions, machine_symbol)
        }
        ExpressionNode::Member(member) => match expressions.expression(member.receiver) {
            ExpressionNode::Name(path)
                if path.members.count() == 1
                    && path.symbol.is_valid()
                    && path.symbol == machine_symbol =>
            {
                member
                    .member_symbol
                    .is_valid()
                    .then_some(member.member_symbol)
            }
            _ => expression_root_symbol(member.receiver, expressions, machine_symbol),
        },
        ExpressionNode::Name(path) => first_valid_name_path_symbol(path, expressions),
        _ => None,
    }
}

fn first_valid_name_path_symbol(
    path: &omega_typed_trees::expression::TableNamePath,
    expressions: &omega_typed_trees::expression::ExpressionTable,
) -> Option<SymbolHandle> {
    expressions
        .name_path_member_symbols(path.member_symbols)
        .first()
        .copied()
        .filter(|symbol| symbol.is_valid())
        .or_else(|| path.head_symbol.is_valid().then_some(path.head_symbol))
        .or_else(|| path.symbol.is_valid().then_some(path.symbol))
}

#[cfg(test)]
mod tests {
    use super::{
        build_borrow_facts, build_flow_facts, build_proof_facts, build_semantic_facts,
        call_mutated_places, context_proves_requirement_place_domain,
        instantiate_call_contract_place, lower_typed_trees, StateMutationSummaryCache,
    };
    use omega_checked_trees::expression::{CallExpression, Expression, NamePath};
    use omega_checked_trees::machine::{Machine, TraitConformance};
    use omega_checked_trees::name::ProgramName;
    use omega_checked_trees::signature::{
        SignatureContract, SignatureContractKind, StateParameter, StateSignature,
    };
    use omega_checked_trees::state::State;
    use omega_checked_trees::statement::{StatementNode, TableCall};
    use omega_checked_trees::trait_definition::TraitDefinition;
    use omega_checked_trees::types::TypeReferenceNode;
    use omega_checked_trees::{BorrowAccessKind, ContractProofFactKind, ContractProofFactOwner};
    use omega_core::arena::HandleSpan;
    use omega_core::symbols::SymbolHandle;
    use omega_facts::{FactPayload, FactPlace};
    use omega_source_files_to_tokens::Lexer;
    use omega_symbol_resolved_trees_to_typed_trees::lower_symbol_resolved_trees;
    use omega_syntax_trees_to_symbol_resolved_trees::lower_syntax_trees;
    use omega_tokens_to_syntax_trees::parse_syntax_trees;
    use std::sync::Arc;

    #[test]
    fn carries_machine_contract_facts_into_checked_proof_facts() {
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_fact = facts
            .contract_facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("checked proof facts should include the machine contract");

        assert_eq!(facts.contract_facts.len(), 1);
        assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(contract_fact.fact, fact);
        assert_eq!(
            contract_fact.owner,
            ContractProofFactOwner::Machine { machine_symbol }
        );
    }

    #[test]
    fn centralizes_contract_facts_in_semantic_fact_plan() {
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let proof = build_proof_facts(&program, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&program, &proof);

        assert_eq!(semantic.facts.len(), 1);
        assert_eq!(semantic.contexts.len(), 1);
        assert_eq!(semantic.symbol_sets.len(), 0);

        let semantic_fact = semantic
            .facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("semantic contract fact");
        let omega_facts::FactPlace::Place(place) = semantic_fact.place else {
            panic!("expected canonical contract fact place");
        };
        assert_eq!(
            semantic.places.get(place).root,
            omega_facts::PlaceRoot::Expression(expression)
        );
        assert_eq!(
            semantic_fact.payload,
            omega_facts::FactPayload::ContractBooleanExpression {
                kind: omega_facts::ContractFactKind::Requires,
                fact,
                expression,
            }
        );
        let context = semantic
            .contexts_at_point(omega_facts::ProgramPoint::Machine { machine_symbol })
            .next()
            .expect("machine contract context");
        assert_eq!(context.boolean_facts().count(), 1);
    }

    #[test]
    fn builds_shared_flow_facts_for_state_and_call_sites() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(40);
        let caller_state_symbol = SymbolHandle::from_arena_index(41);
        let callee_machine_symbol = SymbolHandle::from_arena_index(42);
        let callee_state_symbol = SymbolHandle::from_arena_index(43);

        let mut program = omega_typed_trees::TypedTrees::default();
        let contract_expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let contract_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    contract_expression,
                ));

        let callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("run"),
            parameters: Default::default(),
            return_type: Default::default(),
            statement_nodes: Default::default(),
        };
        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Worker::run"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine_contract(
            &mut callee_machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(contract_fact, 1),
                token_count: 1,
            },
        );
        program.push_machine(callee_machine);

        let call_arguments = HandleSpan::empty();
        let call_statement_receiver = HandleSpan::empty();
        let call_statement = StatementNode::Call(TableCall {
            receiver: call_statement_receiver,
            receiver_symbol: caller_machine_symbol,
            target: ProgramName::generated("run"),
            target_symbol: callee_state_symbol,
            arguments: call_arguments,
        });
        let caller_statement = program.statement_table.insert(call_statement);
        let caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: Default::default(),
            statement_nodes: HandleSpan::from_parts(caller_statement, 1),
        };
        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let effects = omega_effects::infer_effects(&program);
        let borrow = build_borrow_facts(&program);
        let proof = build_proof_facts(&program, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&program, &proof);
        let flow = build_flow_facts(&program, &borrow, &proof, &semantic, &effects);

        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == caller_machine_symbol
                    && state.state_symbol == caller_state_symbol)
                    .then_some(state)
            })
            .expect("caller flow state");
        assert!(caller_flow.entry_semantic_contexts.is_empty());
        assert_eq!(flow.calls.span_or_empty(caller_flow.calls).len(), 1);

        let call_flow = flow.calls.span_or_empty(caller_flow.calls)[0].clone();
        assert_eq!(call_flow.statement_index, 0);
        assert_eq!(call_flow.call_ordinal, 0);
        assert_eq!(call_flow.target_symbol, callee_state_symbol);
        assert!(call_flow.entry_semantic_contexts.is_empty());
        assert!(!call_flow.requires_contexts.is_empty());
        assert!(call_flow.exit_semantic_contexts.is_empty());
        assert_eq!(
            proof
                .contract_fact_refs
                .span_or_empty(call_flow.requires)
                .len(),
            1
        );
    }

    #[test]
    fn invalidates_proved_domain_membership_after_mutating_call() {
        let source = r#"
            data Player {
                health: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            data Main {
                player: Player;
            }

            machine Main::mark_valid(&mut self, player: &mut Player)
            ensures
                player in Player::Valid
            {
                player.health = 0;
            }

            machine Main::break_valid(&mut self, player: &mut Player) {
                player.health = 200;
            }

            machine Main::heal(&mut self, player: &mut Player)
            requires
                player in Player::Valid
            {
                player.health = 10;
            }

            machine Main::main(&mut self) {
                self.mark_valid(&mut self.player);
                self.break_valid(&mut self.player);
                self.heal(&mut self.player);
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let effects = omega_effects::infer_effects(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &effects);
        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_state = typed
            .machine_states(main_machine)
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("main state");
        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state)
            })
            .expect("main flow state");
        let calls = flow.calls.span_or_empty(caller_flow.calls);
        assert_eq!(calls.len(), 3);

        let heal_call = &calls[2];
        let (required_place, required_domain) = flow
            .semantic_context_refs
            .span_or_empty(heal_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic
                    .context_view(context)
                    .facts()
                    .find_map(|fact| match fact.payload {
                        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                            let FactPlace::Place(place) = fact.place else {
                                return None;
                            };
                            Some((place, domain_symbol))
                        }
                        _ => None,
                    })
            })
            .expect("heal requires domain membership");

        let mark_exit_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[0].exit_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });
        let break_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[1].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });
        let heal_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[2].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });

        let diagnostics =
            lower_typed_trees(typed.clone()).expect_err("requires should fail after mutation");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call heal from Main::main")
        }));

        assert!(mark_exit_proves);
        assert!(break_entry_proves);
        assert!(!heal_entry_proves);
    }

    #[test]
    fn invalidates_imported_domain_requires_after_mutating_call() {
        let source = r#"
            data Player {
                health: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            domain Player::Alive {
                self in Player::Valid;
                self.health > 0;
            }

            data Main {
                player: Player;
            }

            machine Main::mark_valid(&mut self, player: &mut Player)
            ensures
                player in Player::Valid
            {
                player.health = 0;
            }

            machine Main::break_valid(&mut self, player: &mut Player) {
                player.health = 200;
            }

            machine Main::heal(&mut self, player: &mut Player)
            requires
                player in Player::Valid
            ensures
                player in Player::Alive
            {
                player.health = 10;
            }

            machine Main::main(&mut self) {
                self.mark_valid(&mut self.player);
                self.break_valid(&mut self.player);
                self.heal(&mut self.player);
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let effects = omega_effects::infer_effects(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &effects);
        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_state = typed
            .machine_states(main_machine)
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("main state");
        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state)
            })
            .expect("main flow state");
        let calls = flow.calls.span_or_empty(caller_flow.calls);
        let heal_call = &calls[2];
        let (required_place, required_domain) = flow
            .semantic_context_refs
            .span_or_empty(heal_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic
                    .context_view(context)
                    .facts()
                    .find_map(|fact| match fact.payload {
                        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                            let FactPlace::Place(place) = fact.place else {
                                return None;
                            };
                            Some((place, domain_symbol))
                        }
                        _ => None,
                    })
            })
            .expect("heal requires domain membership");
        let heal_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[2].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });
        assert!(!heal_entry_proves);

        let diagnostics =
            lower_typed_trees(typed).expect_err("requires should fail after mutation");
        assert!(diagnostics.iter().any(|diagnostic| {
            diagnostic
                .message
                .contains("cannot prove requires contract for call heal from Main::main")
        }));
    }

    #[test]
    fn preserves_imported_domain_requires_across_disjoint_mutating_call() {
        let source = r#"
            data Player {
                health: i32;
                mana: i32;
                stamina: i32;
            }

            domain Player::Valid {
                self.health >= 0;
                self.health <= 100;
            }

            domain Player::Ready {
                self in Player::Valid;
                self.mana >= 0;
            }

            data Main {
                player: Player;
            }

            machine Main::mark_ready(&mut self, player: &mut Player)
            ensures
                player in Player::Ready
            {
                player.health = 40;
                player.mana = 5;
            }

            machine Main::spend_stamina(&mut self, player: &mut Player) {
                player.stamina = 0;
            }

            machine Main::heal(&mut self, player: &mut Player)
            requires
                player in Player::Ready
            {
                player.health = 50;
            }

            machine Main::main(&mut self) {
                self.mark_ready(&mut self.player);
                self.spend_stamina(&mut self.player);
                self.heal(&mut self.player);
            }
        "#;

        let tokens = Lexer::new(source).tokenize().expect("tokenize");
        let syntax = parse_syntax_trees(&tokens).expect("parse");
        let resolved = lower_syntax_trees(&syntax).expect("resolve");
        let typed = lower_symbol_resolved_trees(&resolved).expect("type");
        let proof_plan = omega_proof::obligations::build_proof_plan(&typed);
        let effects = omega_effects::infer_effects(&typed);
        let borrow = build_borrow_facts(&typed);
        let proof = build_proof_facts(&typed, &proof_plan, &borrow);
        let semantic = build_semantic_facts(&typed, &proof);
        let flow = build_flow_facts(&typed, &borrow, &proof, &semantic, &effects);
        let main_machine = typed
            .machines()
            .iter()
            .find(|machine| machine.name.as_str() == "Main::main")
            .expect("main machine");
        let main_state = typed
            .machine_states(main_machine)
            .iter()
            .find(|state| state.name.as_str() == "main")
            .expect("main state");
        let caller_flow = flow
            .states
            .iter()
            .find_map(|(_, state)| {
                (state.machine_symbol == main_machine.symbol
                    && state.state_symbol == main_state.symbol)
                    .then_some(state)
            })
            .expect("main flow state");
        let calls = flow.calls.span_or_empty(caller_flow.calls);
        let heal_call = &calls[2];
        let (required_place, required_domain) = flow
            .semantic_context_refs
            .span_or_empty(heal_call.requires_contexts)
            .iter()
            .find_map(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                semantic
                    .context_view(context)
                    .facts()
                    .find_map(|fact| match fact.payload {
                        FactPayload::ContractDomainMembership { domain_symbol, .. } => {
                            let FactPlace::Place(place) = fact.place else {
                                return None;
                            };
                            Some((place, domain_symbol))
                        }
                        _ => None,
                    })
            })
            .expect("heal requires domain membership");
        let heal_entry_proves = flow
            .semantic_context_refs
            .span_or_empty(calls[2].entry_semantic_contexts)
            .iter()
            .any(|context_ref| {
                let context = semantic.contexts.get(context_ref.context);
                context_proves_requirement_place_domain(
                    &typed,
                    &semantic,
                    context,
                    required_place,
                    required_domain,
                )
            });

        assert!(heal_entry_proves);
        lower_typed_trees(typed).expect("disjoint mutation should preserve imported domain fact");
    }

    #[test]
    fn instantiates_call_contract_places_onto_caller_arguments() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(1);
        let caller_state_symbol = SymbolHandle::from_arena_index(2);
        let callee_machine_symbol = SymbolHandle::from_arena_index(3);
        let callee_state_symbol = SymbolHandle::from_arena_index(4);
        let caller_argument_symbol = SymbolHandle::from_arena_index(5);
        let callee_parameter_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let caller_argument_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_typed_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: caller_argument_symbol,
                        symbol: caller_argument_symbol,
                    },
                ));
        let callee_parameter_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_typed_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: callee_parameter_symbol,
                        symbol: callee_parameter_symbol,
                    },
                ));
        let callee_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    callee_parameter_expression,
                ));

        let mut caller_arguments = HandleSpan::empty();
        program
            .statement_table
            .push_expression_handle(&mut caller_arguments, caller_argument_expression);
        let caller_statement = program
            .statement_table
            .insert(StatementNode::Call(TableCall {
                receiver_symbol: SymbolHandle::invalid(),
                target_symbol: callee_state_symbol,
                receiver: HandleSpan::empty(),
                target: ProgramName::generated("run"),
                arguments: caller_arguments,
            }));

        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: HandleSpan::empty(),
            return_type: Default::default(),
            statement_nodes: HandleSpan::from_parts(caller_statement, 1),
        };
        program.push_state_parameter(
            &mut caller_state,
            StateParameter {
                symbol: caller_argument_symbol,
                name: ProgramName::generated("value"),
                type_reference: Default::default(),
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Caller"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let mut callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("run"),
            parameters: HandleSpan::empty(),
            return_type: Default::default(),
            statement_nodes: HandleSpan::empty(),
        };
        program.push_state_parameter(
            &mut callee_state,
            StateParameter {
                symbol: callee_parameter_symbol,
                name: ProgramName::generated("amount"),
                type_reference: Default::default(),
                is_const: false,
                is_mutable: false,
                is_self: false,
            },
        );

        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Worker"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine(callee_machine);

        let call = omega_checked_trees::ContractCallFact {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index: 0,
            call_ordinal: 0,
            target_machine_symbol: callee_machine_symbol,
            target_state_symbol: callee_state_symbol,
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        };
        let contract = omega_checked_trees::ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: callee_machine_symbol,
                state_symbol: callee_state_symbol,
            },
            fact: callee_fact,
        };

        let mut semantic = omega_facts::FactPlan::default();
        let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
        let omega_facts::FactPlace::Place(place_handle) = place else {
            panic!("expected instantiated call place");
        };

        assert_eq!(
            semantic.places.get(place_handle).root,
            omega_facts::PlaceRoot::Symbol(caller_argument_symbol)
        );
    }

    #[test]
    fn carries_trait_signature_contract_facts_into_checked_proof_facts() {
        let trait_symbol = SymbolHandle::from_arena_index(5);
        let signature_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));

        let mut trait_definition = TraitDefinition {
            symbol: trait_symbol,
            is_boundary: true,
            name: ProgramName::generated("Console"),
            requires: Default::default(),
            machines: Default::default(),
        };
        let mut signature = StateSignature {
            symbol: signature_symbol,
            name: ProgramName::generated("write_line"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            effects: Default::default(),
            contracts: Default::default(),
        };
        program.push_state_signature_contract(
            &mut signature,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_trait_machine_signature(&mut trait_definition, signature);
        program.push_trait_definition(trait_definition);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_fact = facts
            .contract_facts
            .iter()
            .next()
            .map(|(_, fact)| fact)
            .expect("checked proof facts should include the trait signature contract");

        assert_eq!(facts.contract_facts.len(), 1);
        assert_eq!(contract_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(contract_fact.fact, fact);
        assert_eq!(
            contract_fact.owner,
            ContractProofFactOwner::StateSignature {
                owner_symbol: trait_symbol,
                state_symbol: signature_symbol,
            }
        );
    }

    #[test]
    fn indexes_call_contract_facts_by_target_machine() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(5);
        let caller_state_symbol = SymbolHandle::from_arena_index(6);
        let target_machine_symbol = SymbolHandle::from_arena_index(7);
        let target_state_symbol = SymbolHandle::from_arena_index(8);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));

        let mut target_machine = Machine {
            symbol: target_machine_symbol,
            name: ProgramName::generated("Target"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut target_machine,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_machine_state(
            &mut target_machine,
            State {
                symbol: target_state_symbol,
                name: ProgramName::generated("run"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(target_machine);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Caller"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        let mut receiver = HandleSpan::empty();
        program
            .statement_table
            .push_name_path_member(&mut receiver, ProgramName::generated("target"));
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: target_machine_symbol,
                target_symbol: target_state_symbol,
                receiver,
                target: ProgramName::generated("run"),
                arguments: Default::default(),
            }),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_call = facts
            .contract_calls
            .iter()
            .next()
            .map(|(_, call)| call)
            .expect("checked proof facts should index the call contract");
        let requires = facts
            .contract_fact_refs
            .span_or_empty(contract_call.requires);

        assert_eq!(facts.contract_calls.len(), 1);
        assert_eq!(contract_call.caller_machine_symbol, caller_machine_symbol);
        assert_eq!(contract_call.caller_state_symbol, caller_state_symbol);
        assert_eq!(contract_call.statement_index, 0);
        assert_eq!(contract_call.call_ordinal, 0);
        assert_eq!(contract_call.target_machine_symbol, target_machine_symbol);
        assert_eq!(contract_call.target_state_symbol, target_state_symbol);
        assert_eq!(requires.len(), 1);
        assert_eq!(facts.contract_facts.get(requires[0].fact).fact, fact);
    }

    #[test]
    fn indexes_inherited_trait_contracts_by_concrete_call_target() {
        let trait_symbol = SymbolHandle::from_arena_index(5);
        let signature_symbol = SymbolHandle::from_arena_index(6);
        let target_machine_symbol = SymbolHandle::from_arena_index(7);
        let target_state_symbol = SymbolHandle::from_arena_index(8);
        let caller_machine_symbol = SymbolHandle::from_arena_index(9);
        let caller_state_symbol = SymbolHandle::from_arena_index(10);

        let mut program = omega_typed_trees::TypedTrees::default();
        let expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(expression));

        let mut trait_definition = TraitDefinition {
            symbol: trait_symbol,
            is_boundary: true,
            name: ProgramName::generated("Drawable"),
            requires: Default::default(),
            machines: Default::default(),
        };
        let mut signature = StateSignature {
            symbol: signature_symbol,
            name: ProgramName::generated("draw"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            effects: Default::default(),
            contracts: Default::default(),
        };
        program.push_state_signature_contract(
            &mut signature,
            SignatureContract {
                kind: SignatureContractKind::Requires,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );
        program.push_trait_machine_signature(&mut trait_definition, signature);
        program.push_trait_definition(trait_definition);

        let mut target_machine = Machine {
            symbol: target_machine_symbol,
            name: ProgramName::generated("Sprite"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_trait_conformance(
            &mut target_machine,
            TraitConformance {
                symbol: trait_symbol,
                name: ProgramName::generated("Drawable"),
            },
        );
        program.push_machine_state(
            &mut target_machine,
            State {
                symbol: target_state_symbol,
                name: ProgramName::generated("draw"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(target_machine);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        let mut receiver = HandleSpan::empty();
        program
            .statement_table
            .push_name_path_member(&mut receiver, ProgramName::generated("sprite"));
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: target_machine_symbol,
                target_symbol: target_state_symbol,
                receiver,
                target: ProgramName::generated("draw"),
                arguments: Default::default(),
            }),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let contract_call = facts
            .contract_calls
            .iter()
            .next()
            .map(|(_, call)| call)
            .expect("checked proof facts should index inherited trait contracts");
        let requires = facts
            .contract_fact_refs
            .span_or_empty(contract_call.requires);

        assert_eq!(facts.contract_calls.len(), 1);
        assert_eq!(requires.len(), 1);
        let inherited_fact = facts.contract_facts.get(requires[0].fact);
        assert_eq!(inherited_fact.kind, ContractProofFactKind::Requires);
        assert_eq!(inherited_fact.fact, fact);
        assert_eq!(
            inherited_fact.owner,
            ContractProofFactOwner::MachineState {
                machine_symbol: target_machine_symbol,
                state_symbol: target_state_symbol,
            }
        );
    }

    #[test]
    fn indexes_terminal_state_contract_ensures() {
        let machine_symbol = SymbolHandle::from_arena_index(5);
        let state_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let fact_expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Boolean(true));
        let fact = program
            .proof_facts
            .append(omega_typed_trees::domain::ProofFact::Expression(
                fact_expression,
            ));
        let return_expression = program
            .expression_table
            .insert(omega_typed_trees::expression::ExpressionNode::Integer(0));

        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main::main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        program.push_machine_contract(
            &mut machine,
            SignatureContract {
                kind: SignatureContractKind::Ensures,
                facts: HandleSpan::from_parts(fact, 1),
                token_count: 1,
            },
        );

        let mut state = State {
            symbol: state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Expression(return_expression),
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine(machine);

        let proof_plan = omega_proof::obligations::build_proof_plan(&program);
        let borrow = build_borrow_facts(&program);
        let facts = build_proof_facts(&program, &proof_plan, &borrow);
        let exit = facts
            .contract_exits
            .iter()
            .next()
            .map(|(_, exit)| exit)
            .expect("checked proof facts should index the exit contract");
        let ensures = facts.contract_fact_refs.span_or_empty(exit.ensures);

        assert_eq!(facts.contract_exits.len(), 1);
        assert_eq!(exit.machine_symbol, machine_symbol);
        assert_eq!(exit.state_symbol, state_symbol);
        assert_eq!(exit.statement_index, 0);
        assert_eq!(ensures.len(), 1);
        assert_eq!(facts.contract_facts.get(ensures[0].fact).fact, fact);
    }

    #[test]
    fn collects_nested_state_call_ordinals_for_checked_borrow_facts() {
        let entry_symbol = SymbolHandle::from_arena_index(1);
        let outer_symbol = SymbolHandle::from_arena_index(2);
        let inner_symbol = SymbolHandle::from_arena_index(3);
        let item_symbol = SymbolHandle::from_arena_index(4);
        let machine_symbol = SymbolHandle::from_arena_index(5);

        let item_argument = Expression::Mutable(Box::new(Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("item")],
            item_symbol,
            item_symbol,
        ))));

        let nested_call = Expression::Call(Box::new(CallExpression {
            receiver: None,
            target_symbol: inner_symbol,
            target: ProgramName::generated("inner"),
            arguments: Arc::from(vec![item_argument].into_boxed_slice()),
        }));

        let mut program = omega_typed_trees::TypedTrees::default();
        let unit_type = program.type_reference_table.insert(TypeReferenceNode::Unit);
        let nested_call = program.expression_table.insert_tree(&nested_call);
        let mut outer_arguments = Default::default();
        program
            .statement_table
            .push_expression_handle(&mut outer_arguments, nested_call);
        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut entry_state = State {
            symbol: entry_symbol,
            name: ProgramName::generated("entry"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut entry_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: SymbolHandle::invalid(),
                target_symbol: outer_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("outer"),
                arguments: outer_arguments,
            }),
        );
        program.push_state_parameter(
            &mut entry_state,
            StateParameter {
                symbol: item_symbol,
                name: ProgramName::generated("item"),
                type_reference: unit_type,
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut machine, entry_state);
        program.push_machine_state(
            &mut machine,
            State {
                symbol: outer_symbol,
                name: ProgramName::generated("outer"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine_state(
            &mut machine,
            State {
                symbol: inner_symbol,
                name: ProgramName::generated("inner"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let calls = facts.calls.span(state.calls).unwrap();

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].statement_index, 0);
        assert_eq!(calls[0].call_ordinal, 0);
        assert_eq!(calls[0].target_symbol, outer_symbol);
        assert_eq!(calls[1].statement_index, 0);
        assert_eq!(calls[1].call_ordinal, 1);
        assert_eq!(calls[1].target_symbol, inner_symbol);
    }

    #[test]
    fn collects_mutable_attached_data_argument_access_roots() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let target_symbol = SymbolHandle::from_arena_index(3);
        let player_symbol = SymbolHandle::from_arena_index(4);

        let mut program = omega_typed_trees::TypedTrees::default();
        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            machine_symbol,
            machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let player_argument = program.expression_table.insert_tree(&player_argument);

        let mut arguments = HandleSpan::empty();
        program
            .statement_table
            .push_expression_handle(&mut arguments, player_argument);

        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut state = State {
            symbol: state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: machine_symbol,
                target_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("heal"),
                arguments,
            }),
        );
        program.push_machine_state(&mut machine, state);
        program.push_machine_state(
            &mut machine,
            State {
                symbol: target_symbol,
                name: ProgramName::generated("heal"),
                parameters: Default::default(),
                return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                statement_nodes: Default::default(),
            },
        );
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let call = facts.calls.span(state.calls).unwrap()[0].clone();
        let accesses = facts.argument_accesses.span(call.accesses).unwrap();

        assert_eq!(accesses.len(), 1);
        assert_eq!(accesses[0].root_symbol, player_symbol);
        assert_eq!(accesses[0].kind, BorrowAccessKind::Mutable);
    }

    #[test]
    fn call_mutated_places_include_mutable_attached_data_arguments() {
        let machine_symbol = SymbolHandle::from_arena_index(1);
        let state_symbol = SymbolHandle::from_arena_index(2);
        let target_symbol = SymbolHandle::from_arena_index(3);
        let player_symbol = SymbolHandle::from_arena_index(4);

        let mut program = omega_typed_trees::TypedTrees::default();
        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            machine_symbol,
            machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let player_argument = program.expression_table.insert_tree(&player_argument);

        let mut arguments = HandleSpan::empty();
        program
            .statement_table
            .push_expression_handle(&mut arguments, player_argument);

        let mut machine = Machine {
            symbol: machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut state = State {
            symbol: state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: machine_symbol,
                target_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("heal"),
                arguments,
            }),
        );
        program.push_machine_state(&mut machine, state);
        let mut target_state = State {
            symbol: target_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.push_state_parameter(
            &mut target_state,
            StateParameter {
                symbol: SymbolHandle::from_arena_index(5),
                name: ProgramName::generated("self"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: true,
            },
        );
        program.push_state_parameter(
            &mut target_state,
            StateParameter {
                symbol: SymbolHandle::from_arena_index(6),
                name: ProgramName::generated("player"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut machine, target_state);
        program.push_machine(machine);

        let facts = build_borrow_facts(&program);
        let state = facts.states.iter().next().map(|(_, state)| state).unwrap();
        let call = facts.calls.span(state.calls).unwrap()[0].clone();
        let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
        let places = call_mutated_places(
            &program,
            machine_symbol,
            state_symbol,
            &facts,
            &call,
            &mut state_mutation_summary_cache,
        );

        assert!(places.iter().any(|place| place.root
            == omega_facts::PlaceRoot::Symbol(player_symbol)
            && place.segments.is_empty()));
    }

    #[test]
    fn instantiates_call_contract_places_for_attached_data_arguments() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(1);
        let caller_state_symbol = SymbolHandle::from_arena_index(2);
        let callee_machine_symbol = SymbolHandle::from_arena_index(3);
        let callee_state_symbol = SymbolHandle::from_arena_index(4);
        let caller_player_symbol = SymbolHandle::from_arena_index(5);
        let callee_player_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let player_fact_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_checked_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: callee_player_symbol,
                        symbol: callee_player_symbol,
                    },
                ));
        let callee_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    player_fact_expression,
                ));

        let mut caller_arguments = HandleSpan::empty();
        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            caller_machine_symbol,
            caller_machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: caller_player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let player_argument = program.expression_table.insert_tree(&player_argument);
        program
            .statement_table
            .push_expression_handle(&mut caller_arguments, player_argument);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Call(TableCall {
                receiver_symbol: caller_machine_symbol,
                target_symbol: callee_state_symbol,
                receiver: Default::default(),
                target: ProgramName::generated("heal"),
                arguments: caller_arguments,
            }),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.push_state_parameter(
            &mut callee_state,
            StateParameter {
                symbol: callee_player_symbol,
                name: ProgramName::generated("player"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine(callee_machine);

        let call = omega_checked_trees::ContractCallFact {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index: 0,
            call_ordinal: 0,
            target_machine_symbol: callee_machine_symbol,
            target_state_symbol: callee_state_symbol,
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        };
        let contract = omega_checked_trees::ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: callee_machine_symbol,
                state_symbol: callee_state_symbol,
            },
            fact: callee_fact,
        };

        let mut semantic = omega_facts::FactPlan::default();
        let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
        let omega_facts::FactPlace::Place(place_handle) = place else {
            panic!("expected instantiated call place");
        };
        let place = semantic.places.get(place_handle);
        let segments = semantic.place_segments.span_or_empty(place.segments);

        assert_eq!(
            place.root,
            omega_facts::PlaceRoot::Symbol(caller_machine_symbol)
        );
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0],
            omega_facts::PlaceSegment::Field {
                symbol: caller_player_symbol
            }
        );
    }

    #[test]
    fn instantiates_call_contract_places_for_expression_statement_calls() {
        let caller_machine_symbol = SymbolHandle::from_arena_index(1);
        let caller_state_symbol = SymbolHandle::from_arena_index(2);
        let callee_machine_symbol = SymbolHandle::from_arena_index(3);
        let callee_state_symbol = SymbolHandle::from_arena_index(4);
        let caller_player_symbol = SymbolHandle::from_arena_index(5);
        let callee_player_symbol = SymbolHandle::from_arena_index(6);

        let mut program = omega_typed_trees::TypedTrees::default();
        let player_fact_expression =
            program
                .expression_table
                .insert(omega_typed_trees::expression::ExpressionNode::Name(
                    omega_checked_trees::expression::TableNamePath {
                        members: HandleSpan::empty(),
                        member_symbols: HandleSpan::empty(),
                        head_symbol: callee_player_symbol,
                        symbol: callee_player_symbol,
                    },
                ));
        let callee_fact =
            program
                .proof_facts
                .append(omega_typed_trees::domain::ProofFact::Expression(
                    player_fact_expression,
                ));

        let self_name = Expression::Name(NamePath::resolved(
            vec![ProgramName::generated("self")],
            caller_machine_symbol,
            caller_machine_symbol,
        ));
        let player_member = Expression::Member(Box::new(
            omega_checked_trees::expression::MemberExpression {
                receiver: self_name,
                member_symbol: caller_player_symbol,
                member: ProgramName::generated("player"),
            },
        ));
        let player_argument = Expression::Mutable(Box::new(player_member));
        let call_expression = Expression::Call(Box::new(CallExpression {
            receiver: Some(Box::new(Expression::Name(NamePath::resolved(
                vec![ProgramName::generated("self")],
                caller_machine_symbol,
                caller_machine_symbol,
            )))),
            target_symbol: callee_state_symbol,
            target: ProgramName::generated("heal"),
            arguments: Arc::from(vec![player_argument].into_boxed_slice()),
        }));
        let call_expression = program.expression_table.insert_tree(&call_expression);

        let mut caller_machine = Machine {
            symbol: caller_machine_symbol,
            name: ProgramName::generated("Main"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut caller_state = State {
            symbol: caller_state_symbol,
            name: ProgramName::generated("main"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.statement_table.push_statement(
            &mut caller_state.statement_nodes,
            StatementNode::Expression(call_expression),
        );
        program.push_machine_state(&mut caller_machine, caller_state);
        program.push_machine(caller_machine);

        let mut callee_machine = Machine {
            symbol: callee_machine_symbol,
            name: ProgramName::generated("Game"),
            attached_data: None,
            contains: Default::default(),
            owned_data: Default::default(),
            satisfies: Default::default(),
            effects: Default::default(),
            contracts: Default::default(),
            states: Default::default(),
        };
        let mut callee_state = State {
            symbol: callee_state_symbol,
            name: ProgramName::generated("heal"),
            parameters: Default::default(),
            return_type: omega_typed_trees::types::TypeReferenceHandle::invalid(),
            statement_nodes: Default::default(),
        };
        program.push_state_parameter(
            &mut callee_state,
            StateParameter {
                symbol: callee_player_symbol,
                name: ProgramName::generated("player"),
                type_reference: omega_typed_trees::types::TypeReferenceHandle::invalid(),
                is_const: false,
                is_mutable: true,
                is_self: false,
            },
        );
        program.push_machine_state(&mut callee_machine, callee_state);
        program.push_machine(callee_machine);

        let call = omega_checked_trees::ContractCallFact {
            caller_machine_symbol,
            caller_state_symbol,
            statement_index: 0,
            call_ordinal: 0,
            target_machine_symbol: callee_machine_symbol,
            target_state_symbol: callee_state_symbol,
            requires: HandleSpan::empty(),
            ensures: HandleSpan::empty(),
        };
        let contract = omega_checked_trees::ContractProofFact {
            kind: ContractProofFactKind::Requires,
            owner: ContractProofFactOwner::MachineState {
                machine_symbol: callee_machine_symbol,
                state_symbol: callee_state_symbol,
            },
            fact: callee_fact,
        };

        let mut semantic = omega_facts::FactPlan::default();
        let place = instantiate_call_contract_place(&program, &mut semantic, &call, &contract);
        let omega_facts::FactPlace::Place(place_handle) = place else {
            panic!("expected instantiated call place");
        };
        let place = semantic.places.get(place_handle);
        let segments = semantic.place_segments.span_or_empty(place.segments);

        assert_eq!(
            place.root,
            omega_facts::PlaceRoot::Symbol(caller_machine_symbol)
        );
        assert_eq!(segments.len(), 1);
        assert_eq!(
            segments[0],
            omega_facts::PlaceSegment::Field {
                symbol: caller_player_symbol
            }
        );
    }
}
