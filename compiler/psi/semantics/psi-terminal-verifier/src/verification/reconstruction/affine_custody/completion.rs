//! Independent completion of one enumerated affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{IntegerAffineWitness, check_integer_affine_witness};

mod direct;
mod relaxed;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root_bound: &Proposition,
    witness: &IntegerAffineWitness,
) -> bool {
    check_integer_affine_witness(context, semantic_axioms, witness).is_ok_and(|form| {
        direct::retained(&form, root_bound, goal) || relaxed::retained(&form, root_bound, goal)
    })
}
