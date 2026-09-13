//! Producer-local completion of one pre-boundary affine witness.

use proof_admission::{
    IntegerAffineWitness, ProofNode, ProofRule, check_certificate, check_integer_affine_witness,
};
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::relaxation;

pub(super) fn prove(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    maximum_axiom: usize,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
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
    let conclusion = relaxation::mapped_bound(&form, &root_bound.conclusion)?;
    let proof = ProofNode {
        conclusion: conclusion.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness,
        },
    };
    check_certificate(context, &conclusion, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}
