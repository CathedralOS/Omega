mod contracts;
mod points;

use crate::context::*;
use crate::flow::effective_member_symbol;
pub(crate) use crate::semantic_calls::{
    CallSite, call_site_argument_expressions, find_call_site, find_state, find_state_in_machine,
};
pub(crate) use crate::semantic_places::instantiate_call_contract_place;
use crate::semantic_places::{append_place_segment, resolve_place_member_symbol};
use contracts::append_contract_semantic_facts;
use omega_facts::PlaceHandle;
use points::{contract_fact_origin, contract_fact_point, proof_obligation_point};

pub(crate) fn build_semantic_facts(
    program: &omega_typed_trees::TypedTrees,
    proof: &ProofFacts,
) -> FactPlan {
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

pub(crate) fn contract_fact_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
) -> FactPlace {
    match program.proof_facts.get(contract.fact) {
        omega_typed_trees::domain::ProofFact::Expression(expression) => FactPlace::Place(
            contract_expression_place(program, facts, contract, *expression)
                .unwrap_or_else(|| facts.append_place_from_expression(program, *expression)),
        ),
        omega_typed_trees::domain::ProofFact::Membership(membership) => FactPlace::Place(
            contract_expression_place(program, facts, contract, membership.value)
                .unwrap_or_else(|| facts.append_place_from_expression(program, membership.value)),
        ),
    }
}

fn contract_expression_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    expression: ExpressionHandle,
) -> Option<PlaceHandle> {
    if !expression.is_valid() {
        return None;
    }

    match program.expression_table.expression(expression) {
        ExpressionNode::Mutable(inner) => {
            contract_expression_place(program, facts, contract, *inner)
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
            let segment = omega_facts::PlaceSegment::Field { symbol };
            Some(append_place_segment(facts, receiver, segment))
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
                omega_facts::PlaceSegment::Index {
                    expression: indexed.index,
                },
            ))
        }
        _ => None,
    }
}

fn contract_name_path_place(
    program: &omega_typed_trees::TypedTrees,
    facts: &mut FactPlan,
    contract: &ContractProofFact,
    path: &omega_typed_trees::expression::TableNamePath,
) -> Option<PlaceHandle> {
    let members = program.expression_table.name_path_members(path.members);
    let member_symbols = program
        .expression_table
        .name_path_member_symbols(path.member_symbols);

    let (mut place, start_index) = if path.head_symbol.is_valid() {
        (facts.append_symbol_place(path.head_symbol), 1usize)
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
        place = append_place_segment(
            facts,
            place,
            omega_facts::PlaceSegment::Field {
                symbol: member_symbol,
            },
        );
    }

    Some(place)
}

fn contract_owner_self_symbol(
    program: &omega_typed_trees::TypedTrees,
    owner: ContractProofFactOwner,
) -> Option<SymbolHandle> {
    let state_symbol = match owner {
        ContractProofFactOwner::MachineState { state_symbol, .. }
        | ContractProofFactOwner::StateSignature { state_symbol, .. } => state_symbol,
        ContractProofFactOwner::Machine { machine_symbol } => {
            return program
                .machines()
                .iter()
                .find(|machine| machine.symbol == machine_symbol)
                .and_then(|machine| {
                    program.machine_states(machine).iter().find_map(|state| {
                        program
                            .state_parameters(state)
                            .iter()
                            .find(|parameter| parameter.is_self)
                            .map(|parameter| parameter.symbol)
                    })
                });
        }
        ContractProofFactOwner::Unknown => return None,
    };

    let state = find_state(program, state_symbol)?;
    program
        .state_parameters(state)
        .iter()
        .find(|parameter| parameter.is_self)
        .map(|parameter| parameter.symbol)
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
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    crate::lower_typed_trees(program)
}
