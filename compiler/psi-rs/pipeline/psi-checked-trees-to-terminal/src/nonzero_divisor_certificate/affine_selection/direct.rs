//! Direct retained affine-root bound proof selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody::DefinitionIndex;

mod candidates;
mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    candidates::find(assumptions, semantic_axioms, |root, root_bound| {
        completion::prove(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
            root,
            root_bound,
        )
    })
}
