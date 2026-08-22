//! Independent direct retained integer-cast root-bound selection.

use psi_core::{Proposition, PropositionContext};

mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
) -> bool {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|root_bound| match root_bound {
            Proposition::LessOrEqual(left, right) => Some((root_bound, left, right)),
            _ => None,
        })
        .any(|(root_bound, root_left, root_right)| {
            completion::retained(
                context,
                goal,
                semantic_axioms,
                root_bound,
                root_left,
                root_right,
            )
        })
}
