//! Producer-local completion of one post-boundary affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{IntegerAffineWitness, ProofNode};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    minimum_axiom: usize,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    (witness
        .definition_axioms
        .iter()
        .all(|&index| index > minimum_axiom)
        && witness
            .literal_axioms
            .iter()
            .flatten()
            .all(|&index| index > minimum_axiom))
    .then(|| {
        super::super::completion::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root_bound,
            witness,
        )
    })
    .flatten()
}
