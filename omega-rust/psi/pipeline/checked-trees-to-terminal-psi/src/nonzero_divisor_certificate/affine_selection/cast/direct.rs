//! Direct cast-root custody for one following affine word.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

mod candidates;
mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    candidates::prove(context, goal, assumptions, semantic_axioms, definitions)
}
