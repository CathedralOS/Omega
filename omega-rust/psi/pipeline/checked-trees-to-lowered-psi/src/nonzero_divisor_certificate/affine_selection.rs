//! Side-local selection of retained evidence for bounded affine proofs.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

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

pub(super) fn prove_without_cast(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    dispatch::prove_without_cast(context, goal, assumptions, semantic_axioms, definitions)
}
