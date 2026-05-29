mod contracts;
mod points;

use crate::context::*;
pub(crate) use crate::semantic_calls::{
    CallSite, call_site_argument_expressions, find_call_site, find_state, find_state_in_machine,
};
pub(crate) use crate::semantic_places::instantiate_call_contract_place;
use contracts::append_contract_semantic_facts;
pub(crate) use contracts::contract_fact_place;
use points::proof_obligation_point;

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

pub fn lower_typed_program(
    program: omega_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<omega_core::diagnostics::Diagnostic>> {
    crate::lower_typed_trees(program)
}
