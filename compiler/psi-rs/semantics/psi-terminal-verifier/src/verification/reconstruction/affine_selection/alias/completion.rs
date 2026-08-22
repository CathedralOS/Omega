//! Independent affine custody for one completed fixed-depth alias walk.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::affine_custody::{self, DefinitionIndex};

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    affine_custody::retained_from_root(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        root_bound,
    )
}
