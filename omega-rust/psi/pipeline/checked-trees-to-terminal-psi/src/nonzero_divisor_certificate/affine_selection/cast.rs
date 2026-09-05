//! Exact cast-adjacent affine proof precedence.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

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
