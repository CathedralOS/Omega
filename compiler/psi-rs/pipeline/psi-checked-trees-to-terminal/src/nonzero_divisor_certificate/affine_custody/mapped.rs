//! Producer-local exact affine mapping strictly before one source boundary.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule, check_certificate, check_integer_affine_witness};

use super::{DefinitionIndex, candidates, relaxation};

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn prove_mapped_to_target_before(
    context: &PropositionContext,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &ProofNode,
) -> Option<ProofNode> {
    candidates::find_target(
        context,
        semantic_axioms,
        definitions,
        root,
        target,
        |witness| {
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
        },
    )
}
