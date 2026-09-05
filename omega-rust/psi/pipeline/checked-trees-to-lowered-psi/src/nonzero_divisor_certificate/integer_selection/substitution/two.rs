//! Fixed two-equality affine endpoint substitution.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

mod selection;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    allow_cast: bool,
) -> Option<ProofNode> {
    selection::prove(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        allow_cast,
    )
}
