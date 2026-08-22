//! Independent affine completion strictly after one source boundary.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::{DefinitionIndex, candidates};

mod completion;

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn retained_from_root_after(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    root_bound: &Proposition,
) -> bool {
    candidates::any(
        context,
        goal,
        semantic_axioms,
        definitions,
        root,
        |witness| {
            completion::retained(
                context,
                goal,
                semantic_axioms,
                minimum_axiom,
                root_bound,
                witness,
            )
        },
    )
}
