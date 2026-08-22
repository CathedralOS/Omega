//! Side-local selection of retained evidence for bounded affine proofs.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod alias;
mod direct;
mod literal;
mod transitive;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    if let Some(proof) = direct::prove(context, goal, assumptions, semantic_axioms) {
        return Some(proof);
    }
    literal::prove_landed_literal_affine_bound(context, goal, assumptions, semantic_axioms)
        .or_else(|| alias::prove_one(context, goal, assumptions, semantic_axioms))
        .or_else(|| {
            transitive::prove_transitively_reconstructed_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| {
            transitive::prove_transitively_alias_substituted_affine_bound(
                context,
                goal,
                assumptions,
                semantic_axioms,
            )
        })
        .or_else(|| alias::prove_two(context, goal, assumptions, semantic_axioms))
}
