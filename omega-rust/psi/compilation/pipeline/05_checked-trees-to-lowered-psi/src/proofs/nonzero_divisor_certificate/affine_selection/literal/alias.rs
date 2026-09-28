//! One-intermediate-alias literal landing for affine certificate production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext};

use super::super::super::affine_custody::DefinitionIndex;

mod candidates;

use super::{completion, root_bounds};

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
