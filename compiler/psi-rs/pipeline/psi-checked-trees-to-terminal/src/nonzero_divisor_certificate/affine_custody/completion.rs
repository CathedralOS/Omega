//! Producer-local completion of one enumerated affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
};

use super::relaxation;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let direct = ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness: witness.clone(),
        },
    };
    if check_certificate(context, goal, assumptions, semantic_axioms, &direct).is_ok() {
        return Some(direct);
    }

    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let relaxed = relaxation::prove(goal, &form, root_bound, witness)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
        .is_ok()
        .then_some(relaxed)
}
