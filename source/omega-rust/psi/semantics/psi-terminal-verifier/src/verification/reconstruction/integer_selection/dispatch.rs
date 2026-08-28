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
    match goal {
        Proposition::Truth => Some(true),
        Proposition::LessOrEqual(_, _) => Some(bound::retained(
            context,
            goal,
            requirements,
            semantic_axioms,
            definitions,
        )),
        _ => None,
    }
}
