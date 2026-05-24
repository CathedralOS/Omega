mod contracts;
mod points;

use super::*;
pub(crate) use crate::semantic_calls::{
    call_site_argument_expressions, find_call_site, find_state, find_state_in_machine, CallSite,
};
pub(crate) use crate::semantic_places::instantiate_call_contract_place;
use contracts::append_contract_semantic_facts;
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
        omega_typed_trees::domain::ProofFact::Expression(expression) => {
            FactPlace::Place(facts.append_place_from_expression(program, *expression))
        }
        omega_typed_trees::domain::ProofFact::Membership(membership) => {
            FactPlace::Place(facts.append_place_from_expression(program, membership.value))
        }
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
