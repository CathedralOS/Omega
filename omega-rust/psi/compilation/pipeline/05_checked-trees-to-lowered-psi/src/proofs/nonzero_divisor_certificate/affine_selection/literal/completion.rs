//! Common affine-custody completion for landed-literal certificates.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::affine_custody::{self, DefinitionIndex};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    root_bounds: [ProofNode; 2],
) -> Option<ProofNode> {
    root_bounds.into_iter().find_map(|root_bound| {
        affine_custody::prove_from_root(
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
