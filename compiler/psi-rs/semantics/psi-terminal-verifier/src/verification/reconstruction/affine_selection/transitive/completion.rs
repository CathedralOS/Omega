//! Independent direct two-citation affine completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::affine_custody::{self, DefinitionIndex};

mod bound;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> bool {
    let root_bound = bound::retained(left, right);
    [left, right]
        .into_iter()
        .filter(|root| matches!(root, ScalarTerm::Value { .. }))
        .any(|root| {
            affine_custody::retained_from_root(
                context,
                goal,
                semantic_axioms,
                definitions,
                root,
                &root_bound,
            )
        })
}
