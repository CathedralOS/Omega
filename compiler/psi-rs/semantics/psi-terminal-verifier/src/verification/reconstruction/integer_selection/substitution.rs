//! Fixed one- and two-equality integer-bound substitution reconstruction.

use psi_core::{Proposition, PropositionContext};

mod one;
mod relation;
mod two;

pub(super) fn retained(
    context: Option<&PropositionContext>,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    one::retained(context, goal, requirements, semantic_axioms)
        || context
            .is_some_and(|context| two::retained(context, goal, requirements, semantic_axioms))
}
