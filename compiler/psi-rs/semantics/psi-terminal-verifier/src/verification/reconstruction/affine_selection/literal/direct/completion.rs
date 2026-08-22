//! Independent direct landed-literal completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    literal: &ScalarTerm,
) -> bool {
    [
        Proposition::LessOrEqual(literal.clone(), root.clone()),
        Proposition::LessOrEqual(root.clone(), literal.clone()),
    ]
    .iter()
    .any(|root_bound| {
        affine_custody::retained_from_root(context, goal, semantic_axioms, root, root_bound)
    })
}
