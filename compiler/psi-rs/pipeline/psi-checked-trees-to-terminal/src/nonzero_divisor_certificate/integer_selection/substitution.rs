//! Fixed one- and two-equality integer-bound substitution proofs.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody::DefinitionIndex;

mod one;
mod relation;
mod two;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    one::prove(context, goal, assumptions, semantic_axioms, definitions)
        .or_else(|| two::prove(context, goal, assumptions, semantic_axioms, definitions))
}
