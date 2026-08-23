//! Exact cast-adjacent affine proof precedence.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody::DefinitionIndex;

mod direct;
mod endpoint;
mod sandwich;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    direct::prove(context, goal, assumptions, semantic_axioms, definitions)
        .or_else(|| sandwich::prove(context, goal, assumptions, semantic_axioms, definitions))
}
