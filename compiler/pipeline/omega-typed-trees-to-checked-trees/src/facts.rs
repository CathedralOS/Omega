use crate::borrow::build_borrow_facts;
use crate::flow::{build_domain_facts, build_flow_facts};
use crate::invariants::build_invariant_facts;
use crate::proof::build_proof_facts;
use crate::semantic::build_semantic_facts;
use crate::values::build_value_facts;
use omega_checked_trees::CheckFacts;
use omega_effects::EffectPlan;
use omega_proof::obligations::ProofPlan;
use omega_typed_trees::TypedTrees;

pub(crate) fn build_check_facts(
    program: &TypedTrees,
    proof_plan: &ProofPlan<'_>,
    effects: EffectPlan,
) -> CheckFacts {
    let borrow = build_borrow_facts(program);
    let values = build_value_facts(program);
    let proof = build_proof_facts(program, proof_plan, &borrow);
    let invariants = build_invariant_facts(program);
    let mut semantic = build_semantic_facts(program, &proof);
    let domains = build_domain_facts(program, &semantic);
    let flow = build_flow_facts(program, &borrow, &proof, &mut semantic, &domains, &effects);

    CheckFacts::with_roots(
        semantic, borrow, proof, values, invariants, domains, effects, flow,
    )
}
