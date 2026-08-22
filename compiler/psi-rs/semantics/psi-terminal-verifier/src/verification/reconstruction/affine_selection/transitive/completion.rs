//! Independent direct two-citation affine completion replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::affine_custody::{self, DefinitionIndex};
use super::super::bounds;

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
    bounds::value_endpoints(left, right).any(|root| {
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
