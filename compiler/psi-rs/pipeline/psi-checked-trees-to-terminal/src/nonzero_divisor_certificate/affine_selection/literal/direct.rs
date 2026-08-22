//! Direct landed-literal affine-root proof construction.

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
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    candidates::DirectLiteralCandidates::new(assumptions, semantic_axioms).find(
        |root, literal, equality_proof| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                root,
                literal,
                equality_proof,
            )
        },
    )
}
