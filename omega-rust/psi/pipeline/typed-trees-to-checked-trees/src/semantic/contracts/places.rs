use crate::context::*;
use crate::flow::effective_member_symbol;
use crate::semantic_calls::call_target_parameters;
use crate::semantic_places::{append_place_segment, resolve_place_member_symbol};
use facts::PlaceHandle;

#[cfg(test)]
mod tests;

pub(crate) fn contract_fact_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        typed_trees::domain::ProofFact::Expression(expression) => FactPlace::Place(
            contract_expression_place(program, facts, contract, *expression)
                .unwrap_or_else(|| facts.append_place_from_expression(program, *expression)),
        ),
        typed_trees::domain::ProofFact::Membership(membership) => FactPlace::Place(
            contract_expression_place(program, facts, contract, membership.value)
                .unwrap_or_else(|| facts.append_place_from_expression(program, membership.value)),
        ),
        typed_trees::domain::ProofFact::Proposition(application) => {
            FactPlace::Place(facts.append_symbol_place(application.proposition))
        }
    }
}

/// Returns every place read by a boolean contract expression. Requires facts
/// are stored in one context with a copy rooted at each dependency, so
/// mutating any operand invalidates the whole assumption rather than leaving
/// a comparison alive under an opaque expression root.
pub(crate) fn contract_fact_dependency_places(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
) -> Vec<PlaceHandle> {
    let mut places = Vec::new();
    for expression in
        crate::contract_occurrences::fact_referenced_occurrences(program, contract.fact)
    {
        if let Some(place) = contract_expression_place(program, facts, contract, expression) {
            push_unique_place(facts, &mut places, place);
        }
    }
    places
}

/// Resolve the structured validity inputs retained for an outcome-specific
/// caller row. Contract occurrences remain typed expression handles until this
/// point; parameter and `result` paths are instantiated through the exact
/// source call rather than reconstructed from normalized display labels.
pub(crate) fn outcome_specific_fact_dependency_places(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    proof: &ProofFacts,
    arm: &checked_trees::OutcomeSpecificArmFact,
    row: &checked_trees::OutcomeSpecificArmRowFact,
) -> Vec<PlaceHandle> {
    let mut places = Vec::new();
    if let Some(result) = crate::semantic_places::canonical_place_to_fact_place_in_state(
        program,
        facts,
        arm.caller_state_symbol,
        arm.statement_index,
        row.validity.result_occurrence,
    ) {
        push_unique_place(facts, &mut places, result);
    }

    let mut calls = proof.contract_calls.iter().filter_map(|(_, call)| {
        (call.caller_machine_symbol == arm.caller_machine_symbol
            && call.caller_state_symbol == arm.caller_state_symbol
            && call.statement_index == arm.result_call_statement_index
            && call.call_ordinal == 0)
            .then_some(call)
    });
    let Some(call) = calls.next() else {
        return places;
    };
    if calls.next().is_some() {
        return places;
    }

    // Evidence interfaces are carrierless: the checked interface/type owns no
    // independent runtime place, loan, or lease. Its structured scope can
    // therefore retain only the concrete proposition occurrences that carry
    // that evidence at this arm. Re-project those roots here (and deduplicate
    // below) so a future interface carrier cannot silently fall back to an
    // identity-label convention.
    let interface_occurrences = row
        .validity
        .evidence_interface_scope
        .as_ref()
        .map(|scope| scope.retained_occurrences.as_slice())
        .unwrap_or(&[]);
    for expression in row
        .validity
        .referenced_occurrences
        .iter()
        .chain(interface_occurrences)
    {
        if let Some(place) = crate::semantic_places::instantiate_outcome_contract_expression_place(
            program,
            facts,
            call,
            arm.statement_index,
            row.validity.result_occurrence,
            *expression,
        ) {
            push_unique_place(facts, &mut places, place);
        }
    }
    places
}

fn push_unique_place(facts: &FactPlan, places: &mut Vec<PlaceHandle>, candidate: PlaceHandle) {
    let candidate_place = facts.places.get(candidate);
    let candidate_segments = facts.place_segments.span_or_empty(candidate_place.segments);
    let duplicate = places.iter().any(|place| {
        let place = facts.places.get(*place);
        place.root == candidate_place.root
            && facts.place_segments.span_or_empty(place.segments) == candidate_segments
    });
    if !duplicate {
        places.push(candidate);
    }
}

fn contract_expression_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Borrow(inner) => {
            contract_expression_place(program, facts, contract, inner.target)
        }
        ExpressionNode::Name(path) => contract_name_path_place(program, facts, contract, path),
        ExpressionNode::Member(member) => {
            let receiver = contract_expression_place(program, facts, contract, member.receiver)
                .unwrap_or_else(|| facts.append_place_from_expression(program, member.receiver));
            let symbol = {
                let symbol = effective_member_symbol(program, member.receiver, member);
                if symbol.is_valid() {
                    symbol
                } else {
                    resolve_place_member_symbol(program, facts, receiver, member.member.as_str())
                        .unwrap_or_else(SymbolHandle::invalid)
                }
            };
            let receiver = if let Some(variant) = facts::payload_variant_for_field(program, symbol)
            {
                append_place_segment(facts, receiver, facts::PlaceSegment::Case { variant })
            } else {
                receiver
            };
            Some(append_place_segment(
                facts,
                receiver,
                facts::PlaceSegment::Field { symbol },
            ))
        }
        ExpressionNode::Indexed(indexed) => {
            let collection =
                contract_expression_place(program, facts, contract, indexed.collection)
                    .unwrap_or_else(|| {
                        facts.append_place_from_expression(program, indexed.collection)
                    });
            Some(append_place_segment(
                facts,
                collection,
                crate::flow::index_place_segment(program, indexed.index),
            ))
        }
        _ => None,
    }
}

fn contract_name_path_place(
    program: &typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    path: &typed_trees::expression::TableNamePath,
) -> Option<PlaceHandle> {
    let members = program.expression_table.name_path_members(path.members);
    let member_symbols = program
        .expression_table
        .name_path_member_symbols(path.member_symbols);

    let (mut place, start_index) = if members
        .first()
        .is_some_and(|member| member.as_str() == "self")
    {
        let self_symbol = contract_owner_self_symbol(program, contract.owner)?;
        // Typed source names retain the machine attachment identity for self.
        // Contract flow instead transports the owner's explicit self formal;
        // keep that declaration root even once name resolution stamps the path.
        let attachment_root = crate::flow::normalized_event_place_root(
            program,
            facts::PlaceRoot::Symbol(self_symbol),
        );
        if path.head_symbol.is_valid()
            && path.head_symbol != self_symbol
            && attachment_root != facts::PlaceRoot::Symbol(path.head_symbol)
        {
            return None;
        }
        (facts.append_symbol_place(self_symbol), 1usize)
    } else if path.head_symbol.is_valid() {
        (facts.append_symbol_place(path.head_symbol), 1usize)
    } else if let Some(parameter_symbol) =
        contract_owner_parameter_symbol(program, contract.owner, path, members.first())
    {
        // A head name that resolves to a parameter of the contract's owner
        // (for example `item` in `requires item in Item::Ready`) roots the
        // place at that parameter, matching how call-site obligations
        // instantiate the same contract against caller arguments.
        (facts.append_symbol_place(parameter_symbol), 1usize)
    } else {
        let self_symbol = contract_owner_self_symbol(program, contract.owner)?;
        let start_index = usize::from(
            members
                .first()
                .is_some_and(|member| member.as_str() == "self"),
        );
        (facts.append_symbol_place(self_symbol), start_index)
    };

    for (offset, member_name) in members.iter().skip(start_index).enumerate() {
        let member_symbol = member_symbols
            .get(offset + start_index)
            .copied()
            .filter(|symbol| symbol.is_valid())
            .or_else(|| resolve_place_member_symbol(program, facts, place, member_name.as_str()))
            .unwrap_or_else(SymbolHandle::invalid);
        if let Some(variant) = facts::payload_variant_for_field(program, member_symbol) {
            place = append_place_segment(facts, place, facts::PlaceSegment::Case { variant });
        }
        place = append_place_segment(
            facts,
            place,
            facts::PlaceSegment::Field {
                symbol: member_symbol,
            },
        );
    }

    Some(place)
}

fn contract_owner_self_symbol(
    program: &typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
) -> Option<SymbolHandle> {
    contract_owner_parameter(program, owner, |parameter| parameter.is_self)
        .map(|parameter| parameter.symbol)
}

/// Resolves a contract name-path head that names one of the owner's
/// parameters (for example `item` in `requires item in Item::Ready`). Returns
/// the parameter symbol so the fact place is rooted at the parameter, which
/// is how call-site obligations instantiate the same contract.
fn contract_owner_parameter_symbol(
    program: &typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
    path: &typed_trees::expression::TableNamePath,
    head_name: Option<&typed_trees::name::Identifier>,
) -> Option<SymbolHandle> {
    let head_name = head_name?.as_str();
    if head_name == "self" {
        return None;
    }

    contract_owner_parameter(program, owner, |parameter| {
        !parameter.is_self
            && (parameter.name.as_str() == head_name
                || (path.symbol.is_valid() && parameter.symbol == path.symbol))
    })
    .map(|parameter| parameter.symbol)
}

fn contract_owner_parameter(
    program: &typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
    mut matches: impl FnMut(&typed_trees::signature::StateParameter) -> bool,
) -> Option<&typed_trees::signature::StateParameter> {
    let state_symbol = match owner {
        ContractProofFactOwner::MachineState { state_symbol, .. }
        | ContractProofFactOwner::StateSignature { state_symbol, .. } => state_symbol,
        ContractProofFactOwner::Machine { machine_symbol } => {
            return program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)
                .and_then(|machine| {
                    program.machine_states(machine).first().and_then(|state| {
                        program
                            .state_parameters(state)
                            .iter()
                            .find(|parameter| matches(parameter))
                    })
                });
        }
        ContractProofFactOwner::OperatorDeclaration { operator_symbol } => {
            return typed_trees::operator::declaration_by_symbol(program, operator_symbol)
                .and_then(|operator| {
                    program
                        .operator_parameters(operator)
                        .iter()
                        .find(|parameter| matches(parameter))
                });
        }
        ContractProofFactOwner::Unknown | ContractProofFactOwner::OperatorUse { .. } => {
            return None;
        }
    };

    // Machine states resolve through their machine; a StateSignature owner can
    // also be a trait machine signature (boundary trait contracts), whose
    // parameters live on the signature itself.
    call_target_parameters(program, state_symbol)?
        .iter()
        .find(|parameter| matches(parameter))
}
