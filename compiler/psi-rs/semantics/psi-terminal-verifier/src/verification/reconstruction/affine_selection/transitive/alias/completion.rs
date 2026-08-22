//! Independent fixed transitive-bound alias completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::{self, DefinitionIndex};

mod bound;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    alias: &ScalarTerm,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> bool {
    let Some(root_bound) = bound::retained(root, alias, left, right) else {
        return false;
    };
    affine_custody::retained_from_root(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        &root_bound,
    )
}
