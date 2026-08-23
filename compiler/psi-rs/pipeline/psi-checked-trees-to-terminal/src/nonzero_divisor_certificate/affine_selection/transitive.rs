//! Fixed two-citation transitive affine evidence construction.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::affine_custody::DefinitionIndex;

mod alias;
mod chains;
mod completion;

use chains::TwoCitationChains;

/// Reconstruct one affine-root bound through exactly two ordered citations and
/// one exact value equality. This deliberately calls the affine constructor
/// directly: it does not recurse through the general integer-bound search, so
/// neither equality chains nor longer order paths are admitted here.
pub(super) fn prove_transitively_alias_substituted_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    alias::prove(context, goal, assumptions, semantic_axioms, definitions)
}

pub(super) fn prove_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    TwoCitationChains::new(assumptions, semantic_axioms).find(
        |left, right, left_proof, right_proof| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                left,
                right,
                left_proof,
                right_proof,
            )
        },
    )
}
