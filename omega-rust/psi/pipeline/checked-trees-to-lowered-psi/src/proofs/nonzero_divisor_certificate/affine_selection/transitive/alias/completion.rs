//! Fixed transitive-bound alias completion for affine production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::{self, DefinitionIndex};

mod bound;

#[allow(clippy::too_many_arguments)]
pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    alias: &ScalarTerm,
    left: &ScalarTerm,
    right: &ScalarTerm,
    left_proof: ProofNode,
    right_proof: ProofNode,
    equality: ProofNode,
) -> Option<ProofNode> {
    let root_bound = bound::prove(root, alias, left, right, left_proof, right_proof, equality)?;
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
