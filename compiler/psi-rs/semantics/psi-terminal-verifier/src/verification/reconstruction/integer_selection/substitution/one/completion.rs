//! Independent completion of one oriented endpoint equality.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::super::super::super::affine_custody::DefinitionIndex;
use super::super::relation;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal_left: &ScalarTerm,
    goal_right: &ScalarTerm,
    old: &ScalarTerm,
    replacement: &ScalarTerm,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    let relation = if old == goal_left {
        Proposition::LessOrEqual(replacement.clone(), goal_right.clone())
    } else if old == goal_right {
        Proposition::LessOrEqual(goal_left.clone(), replacement.clone())
    } else {
        return false;
    };
    relation::retained(
        context,
        &relation,
        requirements,
        semantic_axioms,
        definitions,
    )
}
