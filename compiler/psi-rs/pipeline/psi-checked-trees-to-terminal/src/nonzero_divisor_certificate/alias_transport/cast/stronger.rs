//! Closed-strengthened alias bounds for exact integer-cast completion.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

mod candidates;
mod completion;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    candidates::find(
        assumptions,
        semantic_axioms,
        |root, alias, literal, endpoint, bound, equality| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                alias,
                literal,
                endpoint,
                bound,
                equality,
            )
        },
    )
}
