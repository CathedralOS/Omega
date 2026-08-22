//! Fixed alias-family dispatch for exact integer-cast bounds.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::{alias_transport, cast_custody};

mod two;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    alias_transport::prove_one(assumptions, semantic_axioms, |root, root_bound| {
        cast_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            root,
            root_bound,
        )
    })
    .or_else(|| alias_transport::prove_stronger_cast(context, goal, assumptions, semantic_axioms))
    .or_else(|| {
        alias_transport::prove_landed_literal_cast(context, goal, assumptions, semantic_axioms)
    })
    .or_else(|| two::prove(context, goal, assumptions, semantic_axioms))
}
