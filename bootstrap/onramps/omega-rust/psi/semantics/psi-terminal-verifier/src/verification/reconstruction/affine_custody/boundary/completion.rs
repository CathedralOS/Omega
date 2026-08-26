//! Independent completion of one post-boundary affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::IntegerAffineWitness;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    minimum_axiom: usize,
    root_bound: &Proposition,
    witness: IntegerAffineWitness,
) -> bool {
    witness
        .definition_axioms
        .iter()
        .all(|&index| index > minimum_axiom)
        && witness
            .literal_axioms
            .iter()
            .flatten()
            .all(|&index| index > minimum_axiom)
        && super::super::completion::retained(context, goal, semantic_axioms, root_bound, &witness)
}
