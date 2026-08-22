//! Side-local selection of retained evidence for bounded affine proofs.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::affine_custody::DefinitionIndex;

mod alias;
mod bounds;
mod cast;
mod direct;
mod dispatch;
mod equalities;
mod fact_identity;
mod literal;
mod transitive;
mod value_index;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let definitions = DefinitionIndex::new(semantic_axioms);
    dispatch::prove(context, goal, assumptions, semantic_axioms, &definitions)
}
