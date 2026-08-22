//! Independent direct two-citation affine completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::affine_custody;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> bool {
    let root_bound = Proposition::LessOrEqual(left.clone(), right.clone());
    [left, right]
        .into_iter()
        .filter(|root| matches!(root, ScalarTerm::Value { .. }))
        .any(|root| {
            affine_custody::retained_from_root(context, goal, semantic_axioms, root, &root_bound)
        })
}
