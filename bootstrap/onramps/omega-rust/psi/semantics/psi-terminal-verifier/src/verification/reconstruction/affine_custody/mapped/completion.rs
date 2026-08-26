//! Independent completion of one pre-boundary affine witness.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::{
    IntegerAffineWitness, check_integer_affine_bound_conversion, check_integer_affine_witness,
};

use super::super::relaxation;

pub(super) fn retained(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    maximum_axiom: usize,
    root_bound: &Proposition,
    witness: IntegerAffineWitness,
) -> Option<Proposition> {
    if !witness
        .definition_axioms
        .iter()
        .all(|&index| index < maximum_axiom)
        || !witness
            .literal_axioms
            .iter()
            .flatten()
            .all(|&index| index < maximum_axiom)
    {
        return None;
    }
    let form = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let mapped = relaxation::mapped_bound(&form, root_bound)?;
    check_integer_affine_bound_conversion(&form, root_bound, &mapped)
        .is_ok()
        .then_some(mapped)
}
