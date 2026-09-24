//! Direct landed-literal affine-root proof construction.

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
        |root, literal, equality_proof| {
            completion::prove(
                context,
                goal,
                assumptions,
                semantic_axioms,
                definitions,
                root,
                root_bounds::direct(root, literal, &equality_proof),
            )
        },
    )
}
