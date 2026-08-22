//! Fixed one-intermediate-alias literal completion for affine production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::super::affine_custody::{self, DefinitionIndex};

mod bound;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    alias: &ScalarTerm,
    literal: &ScalarTerm,
    outer_equality: ProofNode,
    inner_equality: ProofNode,
) -> Option<ProofNode> {
    for root_bound in bound::prove(root, alias, literal, &outer_equality, &inner_equality) {
        if let Some(proof) = affine_custody::prove_from_root(
            context,
            goal,
            assumptions,
            semantic_axioms,
            definitions,
            root,
            root_bound,
        ) {
            return Some(proof);
        }
    }
    None
}
