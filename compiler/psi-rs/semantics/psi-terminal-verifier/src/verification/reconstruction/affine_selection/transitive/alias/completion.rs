//! Independent fixed transitive-bound alias completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    alias: &ScalarTerm,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> bool {
    let root_bound = if alias == left {
        Proposition::LessOrEqual(root.clone(), right.clone())
    } else if alias == right {
        Proposition::LessOrEqual(left.clone(), root.clone())
    } else {
        return false;
    };
    affine_custody::retained_from_root(context, goal, semantic_axioms, root, &root_bound)
}
