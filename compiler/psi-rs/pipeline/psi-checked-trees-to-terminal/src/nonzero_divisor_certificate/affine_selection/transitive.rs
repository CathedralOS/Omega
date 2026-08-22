//! Fixed two-citation transitive affine evidence construction.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

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
) -> Option<ProofNode> {
    alias::prove(context, goal, assumptions, semantic_axioms)
}

pub(super) fn prove_transitively_reconstructed_affine_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    TwoCitationChains::new(assumptions, semantic_axioms).find(
        |left_citation, left_fact, right_citation, right_fact| {
            let Proposition::LessOrEqual(left, _) = left_fact else {
                unreachable!("only integer chains are enumerated")
            };
            let Proposition::LessOrEqual(_, right) = right_fact else {
                unreachable!("only integer chains are enumerated")
            };
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                left,
                right,
                left_citation.proof(left_fact),
                right_citation.proof(right_fact),
            )
        },
    )
}
