use crate::labels::semantic_proof_obligation_kind;
use checked_trees::ProofFacts;
use facts::{Fact, FactOrigin, FactPayload, FactPlace, FactPlan, QualificationEvidence};
mod contracts;
mod field_domains;
mod points;
use contracts::append_contract_semantic_facts;
pub(crate) use contracts::{contract_fact_place, inherited_contract_scope_parts};
pub(crate) use contracts::{contract_parameter_position, projected_argument_place};
use field_domains::{append_local_zii_field_domain_facts, append_state_parameter_domain_facts};
use points::proof_obligation_point;

/// One observed property and its storage dependencies share one lifetime.
/// A dependency asserts no property of its own; an overlapping write retires
/// the context through ordinary invalidation instead of leaving a stale row.
pub(crate) fn append_observation_context(
    facts: &mut FactPlan,
    observation: Fact,
    dependencies: &[FactPlace],
) -> facts::FactContextHandle {
    let dependent = facts.append_fact(observation);
    let mut references = arena::HandleSpan::empty();
    facts.append_ref(&mut references, dependent);
    for &place in dependencies {
        let dependency = facts.append_fact(Fact {
            place,
            payload: FactPayload::StorageDependency { dependent },
            ..observation
        });
        facts.append_ref(&mut references, dependency);
    }
    facts.append_context(observation.point, references)
}

pub(crate) fn build_semantic_facts(
    program: &dyn ::validation::CheckingInput,
    proof: &ProofFacts,
    operators: &checked_trees::CheckedOperatorFacts,
) -> FactPlan {
    let mut facts = validation::build_definition_fact_plan(program);
    append_proof_obligation_semantic_facts(proof, &mut facts);
    append_contract_semantic_facts(program, proof, operators, &mut facts);
    // Receivers and named inputs have the same caller-enforced declared domains.
    // Actual zero-initialized storage is established separately below.
    append_state_parameter_domain_facts(program, &mut facts);
    // ZII locals (`let room: Room;`): the zeroed storage already satisfies
    // every declared field domain that admits the empty byte sequence -- the
    // establishment gate -- so those fields are live evidence
    // from the binding onward.
    append_local_zii_field_domain_facts(program, &mut facts);

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
