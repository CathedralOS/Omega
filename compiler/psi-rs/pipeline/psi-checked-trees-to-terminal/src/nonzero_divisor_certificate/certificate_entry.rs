//! Fail-closed entry from untrusted integer proof selection to the kernel.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{ProofNode, check_certificate};

use super::integer_selection;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let proof = integer_selection::build(context, goal, assumptions, semantic_axioms)?;
    check_certificate(context, goal, assumptions, semantic_axioms, &proof)
        .is_ok()
        .then_some(proof)
}
