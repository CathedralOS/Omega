//! Independent direct retained root-bound cast completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::cast_custody;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root_bound: &Proposition,
    root_left: &ScalarTerm,
    root_right: &ScalarTerm,
) -> bool {
    [root_left, root_right]
        .into_iter()
        .filter(|root| matches!(root, ScalarTerm::Value { .. }))
        .any(|root| {
            cast_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
        })
}
