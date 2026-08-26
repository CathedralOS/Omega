//! Fixed one- and two-alias affine-root proof completion.

use psi_core::{Proposition, PropositionContext};
use psi_proof_admission::ProofNode;

use super::super::{affine_custody::DefinitionIndex, alias_transport};

mod completion;

pub(super) fn prove_one(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    alias_transport::prove_one(assumptions, semantic_axioms, |root, root_bound| {
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

/// Transport one exact retained bound through exactly two distinct value
/// equalities before constructing the affine proof. The equality walk is fixed
/// at depth two; this does not recurse or enumerate a general alias graph.
pub(super) fn prove_two(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    alias_transport::prove_two(assumptions, semantic_axioms, |root, root_bound| {
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
