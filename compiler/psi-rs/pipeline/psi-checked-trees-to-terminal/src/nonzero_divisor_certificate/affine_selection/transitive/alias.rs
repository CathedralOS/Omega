//! One exact alias substitution around a fixed two-citation affine root bound.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

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
    candidates::find(
        assumptions,
        semantic_axioms,
        |root, alias, left, right, left_proof, right_proof, equality_proof| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                root,
                alias,
                left,
                right,
                left_proof,
                right_proof,
                equality_proof,
            )
        },
    )
}
