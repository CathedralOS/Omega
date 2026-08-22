//! Independent affine-witness completion for obligation reconstruction.
//!
//! Evidence selection remains in the parent verifier. This module owns the
//! bounded witness frontier, exact mapped bound, and closed relaxation replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

mod candidates;
mod completion;
mod definition_index;
mod frontier;
mod relaxation;

pub(super) use definition_index::DefinitionIndex;

pub(super) fn retained_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    candidates::any(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        |witness| completion::retained(context, goal, semantic_axioms, root_bound, &witness),
    )
}
