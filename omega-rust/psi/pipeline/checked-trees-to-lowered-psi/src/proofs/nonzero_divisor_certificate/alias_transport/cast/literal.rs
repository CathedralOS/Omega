//! Alias-landed literals for exact integer-cast completion.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

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
        |root, alias, literal, root_equality, literal_equality| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                root,
                alias,
                literal,
                root_equality,
                literal_equality,
            )
        },
    )
}
