//! Independent direct landed-literal completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::{self, DefinitionIndex};
use super::super::root_bounds;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    literal: &ScalarTerm,
) -> bool {
    root_bounds::ordered(root, literal)
        .iter()
        .any(|root_bound| {
            affine_custody::retained_from_root(
                context,
                goal,
                semantic_axioms,
                definitions,
                root,
                &root_bound.proposition,
            )
        })
}
