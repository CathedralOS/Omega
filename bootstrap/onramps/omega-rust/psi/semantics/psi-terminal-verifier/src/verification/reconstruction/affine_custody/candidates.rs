//! Ordered affine-witness candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

mod fixed;
mod targets;

pub(super) fn any(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> bool,
) -> bool {
    let mut targets = targets::values(goal);
    let Some(first_target) = targets.next() else {
        return false;
    };
    std::iter::once(first_target).chain(targets).any(|target| {
        let definition_words = frontier::definition_words_to_target(
            context,
            semantic_axioms,
            definitions,
            root,
            target,
        );
        fixed::any(
            context,
            semantic_axioms,
            definitions,
            definition_words.as_ref(),
            root,
            target,
            &mut complete,
        )
    })
}

pub(super) fn find_target_before<T>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
    mut complete: impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let definition_words =
        frontier::definition_words_to_target(context, semantic_axioms, definitions, root, target);
    fixed::find_where(
        context,
        semantic_axioms,
        definitions,
        definition_words.as_ref(),
        root,
        target,
        |word| word.last().is_some_and(|&index| index < maximum_axiom),
        &mut complete,
    )
}

pub(super) fn any_after(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    mut complete: impl FnMut(IntegerAffineWitness) -> bool,
) -> bool {
    let mut targets = targets::values(goal);
    let Some(first_target) = targets.next() else {
        return false;
    };
    std::iter::once(first_target).chain(targets).any(|target| {
        let definition_words = frontier::definition_words_to_target(
            context,
            semantic_axioms,
            definitions,
            root,
            target,
        );
        fixed::any_where(
            context,
            semantic_axioms,
            definitions,
            definition_words.as_ref(),
            root,
            target,
            |word| word.first().is_some_and(|&index| index > minimum_axiom),
            &mut complete,
        )
    })
}
