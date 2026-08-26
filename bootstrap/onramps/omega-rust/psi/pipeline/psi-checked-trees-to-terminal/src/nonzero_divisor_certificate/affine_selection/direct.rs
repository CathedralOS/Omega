//! Direct retained affine-root bound proof selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::affine_custody::{self, DefinitionIndex};

mod candidates;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    candidates::find(assumptions, semantic_axioms, |root, root_bound| {
        affine_custody::prove_from_root(
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
