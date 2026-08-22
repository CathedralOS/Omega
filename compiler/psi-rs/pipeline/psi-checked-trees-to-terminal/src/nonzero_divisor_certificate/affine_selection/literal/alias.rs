//! One-intermediate-alias literal landing for affine certificate production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::ProofNode;

use super::super::super::affine_custody::DefinitionIndex;

mod candidates;

use super::{completion, root_bounds};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
) -> Option<ProofNode> {
    candidates::LiteralAliasCandidates::new(assumptions, semantic_axioms).find(
        |root, alias, literal, outer_equality, inner_equality| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                root,
                root_bounds::one_alias(root, alias, literal, &outer_equality, &inner_equality),
            )
        },
    )
}
