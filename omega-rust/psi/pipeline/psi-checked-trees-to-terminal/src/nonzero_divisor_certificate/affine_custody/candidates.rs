//! Ordered affine-witness candidates for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

mod fixed;
mod targets;

pub(super) fn find<T>(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let mut targets = targets::values(goal);
    let first_target = targets.next()?;
    std::iter::once(first_target)
        .chain(targets)
        .find_map(|target| {
            let definition_words = frontier::definition_words_to_target(
                context,
                semantic_axioms,
                definitions,
                root,
                target,
            );
            fixed::find(
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

pub(in crate::nonzero_divisor_certificate) fn has_target_before(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    target: &ScalarTerm,
    maximum_axiom: usize,
) -> bool {
    frontier::definition_words_to_target(context, semantic_axioms, definitions, root, target)
        .iter()
        .any(|word| word.last().is_some_and(|&index| index < maximum_axiom))
}

pub(super) fn find_after<T>(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
    mut complete: impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let mut targets = targets::values(goal);
    let first_target = targets.next()?;
    std::iter::once(first_target)
        .chain(targets)
        .find_map(|target| {
            let definition_words = frontier::definition_words_to_target(
                context,
                semantic_axioms,
                definitions,
                root,
                target,
            );
            fixed::find_where(
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

pub(in crate::nonzero_divisor_certificate) fn has_target_after(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
    root: &ScalarTerm,
    minimum_axiom: usize,
) -> bool {
    targets::values(goal).any(|target| {
        frontier::definition_words_to_target(context, semantic_axioms, definitions, root, target)
            .iter()
            .any(|word| word.first().is_some_and(|&index| index > minimum_axiom))
    })
}
