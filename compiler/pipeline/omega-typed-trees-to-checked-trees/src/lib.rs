mod checks;
mod labels;

use omega_checked_trees::expression::{ExpressionHandle, ExpressionNode, NamePath};
use omega_checked_trees::name::ProgramName;
use omega_checked_trees::statement::{
    StatementNode, TableCall, TransitionGuardNode, TransitionTargetNode,
};
use omega_checked_trees::{
    BorrowAccessKind, BorrowArgumentAccessFact, BorrowCallFact, BorrowFacts, BorrowRootKind,
    BorrowWritableRootFact, CheckFacts, ContractCallFact, ContractExitFact, ContractProofFact,
    ContractProofFactKind, ContractProofFactOwner, ContractProofFactRef, DomainDependencyFact,
    DomainDependencyPathFact, DomainFacts, FlowCallFact, FlowExitFact, FlowFacts,
    FlowInvalidationFact, FlowInvalidationSource, FlowSemanticContextRef, FlowStateFact,
    InvariantFact, InvariantFacts, Program, ProofFactKind, ProofFacts, ProofObligationFact,
    ProofObligationOwner, StateBorrowFact,
};
use omega_core::arena::{Handle, HandleSpan};
use omega_core::symbols::SymbolHandle;
use omega_facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, FactRef, ProgramPoint};
use std::collections::BTreeSet;

use crate::labels::{semantic_contract_fact_kind, semantic_proof_obligation_kind, symbol_name};

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
    let domains = build_domain_facts(&program, &semantic);
    let flow = build_flow_facts(&program, &borrow, &proof, &semantic, &domains, &effects);
    let facts = CheckFacts {
        semantic,
        proof,
        borrow,
        invariants,
        domains,
        effects,
        flow,
    };
    checks::check_flow_call_contracts(&program, &facts)?;

    Ok(Program {
        typed: program,
        facts,
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

fn build_domain_facts(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
) -> DomainFacts {
    let mut cache = DomainDependencyCache::default();
    let mut segments = omega_core::arena::Arena::new();
    let mut dependency_paths = omega_core::arena::Arena::new();
    let mut dependencies =
        omega_core::arena::Arena::with_capacity(program.domain_definitions().len());

    for domain in program.domain_definitions() {
        let dependency_segments =
            domain_dependency_segments(program, semantic, &mut cache, domain.symbol);
        let mut dependency_span = omega_core::arena::HandleSpan::empty();
        for dependency in dependency_segments {
            let mut segment_span = omega_core::arena::HandleSpan::empty();
            for segment in dependency {
                segments.append_to_span(&mut segment_span, *segment);
            }
            dependency_paths.append_to_span(
                &mut dependency_span,
                DomainDependencyPathFact {
                    segments: segment_span,
                },
            );
        }

        dependencies.append(DomainDependencyFact {
            domain_symbol: domain.symbol,
            dependencies: dependency_span,
        });
    }

    DomainFacts {
        segments,
        dependency_paths,
        dependencies,
    }
}

fn build_flow_facts(
    program: &omega_typed_trees::TypedTrees,
    borrow: &BorrowFacts,
    proof: &ProofFacts,
    semantic: &FactPlan,
    domains: &DomainFacts,
    effects: &omega_effects::EffectPlan,
) -> FlowFacts {
    let mut state_mutation_summary_cache = StateMutationSummaryCache::default();
    let mut semantic_context_refs =
        omega_core::arena::Arena::with_capacity(semantic.contexts.len().saturating_mul(2));
    let mut invalidation_segments = omega_core::arena::Arena::default();
    let mut invalidations = omega_core::arena::Arena::default();
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
            let state_invalidations_start = invalidations.len();

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
                    let call_invalidations_start = invalidations.len();
                    let post_call_contexts =
                        if call_may_mutate_contract_state(program, borrow, borrow_call) {
                            if mutated_places.is_empty() {
                                omega_core::arena::HandleSpan::empty()
                            } else {
                                filter_contexts_after_place_mutations(
                                    program,
                                    semantic,
                                    domains,
                                    &mut semantic_context_refs,
                                    &mut invalidation_segments,
                                    &mut invalidations,
                                    active_contexts,
                                    &mutated_places,
                                    FlowInvalidationSource::Call {
                                        statement_index: borrow_call.statement_index,
                                        call_ordinal: borrow_call.call_ordinal,
                                        target_symbol: borrow_call.target_symbol,
                                    },
                                )
                            }
                        } else {
                            clone_flow_contexts(&mut semantic_context_refs, active_contexts)
                        };
                    let call_invalidations =
                        appended_span_since(&invalidations, call_invalidations_start);
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
                            invalidations: call_invalidations,
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
                        domains,
                        &mut semantic_context_refs,
                        &mut invalidation_segments,
                        &mut invalidations,
                        active_contexts,
                        &[place],
                        FlowInvalidationSource::Statement { statement_index },
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
                invalidations: appended_span_since(&invalidations, state_invalidations_start),
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
        invalidation_segments,
        invalidations,
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

fn appended_span_since<T: Clone + Default + PartialEq + Eq>(
    arena: &omega_core::arena::Arena<T>,
    start_len: usize,
) -> omega_core::arena::HandleSpan<T> {
    let appended = arena.len().saturating_sub(start_len);
    if appended == 0 {
        omega_core::arena::HandleSpan::empty()
    } else {
        omega_core::arena::HandleSpan::from_parts(
            Handle::from_arena_index(
                start_len
                    .checked_add(1)
                    .and_then(|index| index.try_into().ok())
                    .unwrap(),
            ),
            appended.try_into().unwrap(),
        )
    }
}

fn append_place_segments(
    segments_arena: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    segments: &[omega_facts::PlaceSegment],
) -> omega_core::arena::HandleSpan<omega_facts::PlaceSegment> {
    let start_len = segments_arena.len();
    for segment in segments {
        segments_arena.append(*segment);
    }
    appended_span_since(segments_arena, start_len)
}

fn filter_contexts_after_place_mutations(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &DomainFacts,
    semantic_context_refs: &mut omega_core::arena::Arena<FlowSemanticContextRef>,
    invalidation_segments: &mut omega_core::arena::Arena<omega_facts::PlaceSegment>,
    invalidations: &mut omega_core::arena::Arena<FlowInvalidationFact>,
    source: omega_core::arena::HandleSpan<FlowSemanticContextRef>,
    mutated_places: &[CanonicalPlace],
    invalidation_source: FlowInvalidationSource,
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
        let mut invalidated_any = false;
        for fact_ref in semantic.refs.span_or_empty(context.facts) {
            let fact = semantic.facts.get(fact_ref.fact);
            let FactPlace::Place(place) = fact.place else {
                continue;
            };
            let Some((mutated_place, dependency_segments)) = matching_mutation_for_fact_place(
                program,
                semantic,
                domain_dependencies,
                fact,
                place,
                mutated_places,
            ) else {
                continue;
            };

            invalidated_any = true;
            removed_any = true;
            invalidations.append(FlowInvalidationFact {
                source: invalidation_source,
                context: context_ref.context,
                fact: fact_ref.fact,
                mutated_root: mutated_place.root,
                mutated_segments: append_place_segments(
                    invalidation_segments,
                    &mutated_place.segments,
                ),
                dependency_segments: append_place_segments(
                    invalidation_segments,
                    dependency_segments,
                ),
            });
        }

        if !invalidated_any {
            semantic_context_refs.append_to_span(&mut filtered, context_ref);
        }
    }

    if removed_any { filtered } else { source }
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

impl CanonicalPlace {
    fn extend_segments(&mut self, segments: &[omega_facts::PlaceSegment]) {
        self.segments.extend(segments.iter().copied());
    }
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

fn canonical_place_from_semantic_place(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    place: &omega_facts::Place,
) -> Option<CanonicalPlace> {
    let mut canonical = match place.root {
        omega_facts::PlaceRoot::Unknown => return None,
        omega_facts::PlaceRoot::Symbol(symbol) => canonical_place_from_symbol(symbol)?,
        omega_facts::PlaceRoot::Expression(expression) => {
            canonical_place_from_expression(program, expression)?
        }
        omega_facts::PlaceRoot::TypeReference(type_reference) => CanonicalPlace {
            root: omega_facts::PlaceRoot::TypeReference(type_reference),
            segments: Vec::new(),
        },
    };
    canonical.extend_segments(semantic.place_segments.span_or_empty(place.segments));
    Some(canonical)
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

fn matching_mutation_for_fact_place<'a, 'b>(
    program: &omega_typed_trees::TypedTrees,
    semantic: &FactPlan,
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: omega_facts::PlaceHandle,
    mutated_places: &'b [CanonicalPlace],
) -> Option<(&'b CanonicalPlace, &'a [omega_facts::PlaceSegment])> {
    let place = semantic.places.get(fact_place);
    let fact_canonical_place = canonical_place_from_semantic_place(program, semantic, place)?;

    for mutated_place in mutated_places {
        let is_domain_membership = matches!(
            fact.payload,
            FactPayload::DomainMembership { .. } | FactPayload::ContractDomainMembership { .. }
        );
        if let Some(dependency_segments) = domain_membership_matching_dependency(
            domain_dependencies,
            fact,
            &fact_canonical_place,
            mutated_place,
        ) {
            return Some((mutated_place, dependency_segments));
        }

        if is_domain_membership {
            continue;
        }

        if fact_canonical_place.root == mutated_place.root
            && canonical_place_overlaps_segments(
                &fact_canonical_place.segments,
                &mutated_place.segments,
            )
        {
            return Some((mutated_place, &[]));
        }
    }

    None
}

fn domain_membership_matching_dependency<'a>(
    domain_dependencies: &'a DomainFacts,
    fact: &Fact,
    fact_place: &CanonicalPlace,
    mutated_place: &CanonicalPlace,
) -> Option<&'a [omega_facts::PlaceSegment]> {
    let domain_symbol = match fact.payload {
        FactPayload::DomainMembership { domain_symbol, .. }
        | FactPayload::ContractDomainMembership { domain_symbol, .. } => domain_symbol,
        _ => return None,
    };

    if fact_place.root != mutated_place.root {
        return None;
    }

    let Some(domain_dependency) = domain_dependencies.dependency_fact(domain_symbol) else {
        return canonical_place_overlaps_segments(&fact_place.segments, &mutated_place.segments)
            .then_some(&[]);
    };

    if domain_dependency.dependencies.is_empty() {
        return canonical_place_overlaps_segments(&fact_place.segments, &mutated_place.segments)
            .then_some(&[]);
    }

    domain_dependencies
        .dependency_paths(domain_dependency)
        .find(|dependency_segments| {
            canonical_place_overlaps_joined_segments(
                &fact_place.segments,
                dependency_segments,
                &mutated_place.segments,
            )
        })
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
mod tests;
