//! Independent exact affine mapping strictly before one source boundary.

use psi_core::{Proposition, PropositionContext, ScalarTerm};

use super::{DefinitionIndex, candidates};

mod completion;

#[allow(clippy::too_many_arguments)]
pub(in super::super) fn retained_mapped_to_target_before(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    root_bound: &Proposition,
) -> Option<Proposition> {
    candidates::find_target_before(
        context,
        semantic_axioms,
        definitions,
        root,
        target,
        maximum_axiom,
        |witness| {
            completion::retained(context, semantic_axioms, maximum_axiom, root_bound, witness)
        },
    )
}
