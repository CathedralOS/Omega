//! Producer-local direct cited-bound handoff to affine custody.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::affine_custody::{self, DefinitionIndex};

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    affine_custody::prove_from_root(
        context,
        goal,
        assumptions,
        semantic_axioms,
        definitions,
        root,
        root_bound,
    )
}
