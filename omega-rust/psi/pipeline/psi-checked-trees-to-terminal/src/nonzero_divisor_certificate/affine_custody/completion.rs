//! Producer-local completion of one enumerated affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::{IntegerAffineWitness, ProofNode, check_integer_affine_witness};

mod direct;
mod relaxed;

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
    relaxed::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &form,
        root_bound,
        witness,
    )
}
