//! Direct and one-alias landed-literal affine evidence construction.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody::DefinitionIndex;

mod alias;
mod completion;
mod direct;
mod eligibility;
mod root_bounds;

pub(super) fn prove_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = direct::prove(context, goal, assumptions, semantic_axioms, definitions) {
        return Some(proof);
    }
    alias::prove(context, goal, assumptions, semantic_axioms, definitions)
}
