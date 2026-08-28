//! Independent canonical atomic-integer reconstruction dispatch.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;
use super::bound;

pub(super) fn retained_atomic(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<bool> {
    matches!(goal, Proposition::LessOrEqual(_, _))
        .then(|| bound::retained(context, goal, requirements, semantic_axioms, definitions))
}
