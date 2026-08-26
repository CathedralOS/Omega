mod contracts;
mod field_domains;
mod points;

use crate::context::*;
pub(crate) use crate::semantic_calls::{
    CallSite, call_site_argument_expressions, call_target_parameters, call_target_type_parameters,
    find_call_site, find_state, find_state_in_machine,
};
pub(crate) use crate::semantic_places::instantiate_call_contract_place;
use contracts::append_contract_semantic_facts;
pub(crate) use contracts::contract_fact_place;
use field_domains::{
    append_local_case_payload_domain_facts, append_machine_field_domain_facts,
    append_state_parameter_domain_facts,
};
use points::proof_obligation_point;

pub(crate) fn build_semantic_facts(
    program: &psi_typed_trees::TypedTrees,
    proof: &ProofFacts,
) -> FactPlan {
    let mut facts = psi_facts::build_definition_fact_plan(program);
    append_proof_obligation_semantic_facts(proof, &mut facts);
    append_contract_semantic_facts(program, proof, &mut facts);
    // #66 read-narrowing: surface declared field domains as machine entry facts
    // (sound because every write is enforced in-domain by checks::contracts::writes).
    append_machine_field_domain_facts(program, &mut facts);
    // #66: surface declared state-parameter domains as machine entry assumptions
    // (sound because the param's implicit `requires` is caller-enforced).
    append_state_parameter_domain_facts(program, &mut facts);
    // #66: surface a case-constructed local's payload domain (`let cmd =
    // Command::Say { text: "ok" }` -> `cmd.<payload> in Utf8`) so a destructured
    // payload forwarded as a call argument discharges; construction enforcement
    // guarantees soundness, the flow handles invalidation.
    append_local_case_payload_domain_facts(program, &mut facts);

    facts
}

fn append_proof_obligation_semantic_facts(proof: &ProofFacts, facts: &mut FactPlan) {
    for (_, obligation) in proof.obligations.iter() {
        let point = proof_obligation_point(obligation);
        facts.append_fact_context(Fact {
            place: FactPlace::Unknown,
            point,
            origin: FactOrigin::ProofObligation,
            evidence: QualificationEvidence::default(),
            payload: FactPayload::ProofObligation {
                kind: semantic_proof_obligation_kind(obligation.kind.clone()),
            },
        });
    }
}

pub fn lower_typed_program(
    program: psi_typed_trees::TypedTrees,
) -> Result<CheckedTrees, Vec<psi_diagnostics::Diagnostic>> {
    crate::lower_typed_trees(program)
}
