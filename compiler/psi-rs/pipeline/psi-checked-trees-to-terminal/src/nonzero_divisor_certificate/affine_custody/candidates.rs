//! Ordered affine-witness candidates for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

mod fixed;
mod targets;

pub(super) fn find<T>(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let mut targets = targets::values(goal);
    let first_target = targets.next()?;
    let definition_words = frontier::definition_words(context, semantic_axioms, definitions, root);
    std::iter::once(first_target)
        .chain(targets)
        .find_map(|target| {
            fixed::find(
                context,
                semantic_axioms,
                &definition_words,
                root,
                target,
                &mut complete,
            )
        })
}

pub(super) fn find_target<T>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let definition_words = frontier::definition_words(context, semantic_axioms, definitions, root);
    fixed::find(
        context,
        semantic_axioms,
        &definition_words,
        root,
        target,
        &mut complete,
    )
}
