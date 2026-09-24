//! Direct and one-alias landed-literal affine evidence construction.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod alias;
mod completion;
mod direct;
mod root_bounds;

pub(super) fn prove_landed_literal_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    if let Some(proof) = direct::prove(context, goal, assumptions, semantic_axioms, definitions) {
        return Some(proof);
    }
    alias::prove(context, goal, assumptions, semantic_axioms, definitions)
}
