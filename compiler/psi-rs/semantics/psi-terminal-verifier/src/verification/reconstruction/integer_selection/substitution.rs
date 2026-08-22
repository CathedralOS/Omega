//! Fixed one- and two-equality integer-bound substitution reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::affine_custody::DefinitionIndex;

mod one;
mod relation;
mod two;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> bool {
    one::retained(context, goal, requirements, semantic_axioms, definitions)
        || context.is_some_and(|context| {
            two::retained(context, goal, requirements, semantic_axioms, definitions)
        })
}
