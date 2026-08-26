//! Side-local selection of retained evidence for bounded affine proofs.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

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
    let mut definitions = DefinitionIndex::new(semantic_axioms);
    prove_with_definitions(
        context,
        goal,
        assumptions,
        semantic_axioms,
        &mut definitions,
    )
}

pub(super) fn prove_with_definitions(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = definitions.cached_affine_proof(goal) {
        return proof;
    }
    definitions.begin_affine_proof(goal);
    let proof = dispatch::prove(context, goal, assumptions, semantic_axioms, definitions);
    definitions.cache_affine_proof(goal, proof.clone());
    proof
}
