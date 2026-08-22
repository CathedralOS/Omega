//! Producer-local completion of one enumerated affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{IntegerAffineWitness, ProofNode, check_integer_affine_witness};

use super::relaxation;

mod direct;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    if let Some(direct) = direct::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        root_bound,
        &witness,
    ) {
        return Some(direct);
    }

    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let relaxed = relaxation::prove(goal, &form, root_bound, witness)?;
    psi_proof_kernel::check_certificate(context, goal, assumptions, semantic_axioms, &relaxed)
        .is_ok()
        .then_some(relaxed)
}
