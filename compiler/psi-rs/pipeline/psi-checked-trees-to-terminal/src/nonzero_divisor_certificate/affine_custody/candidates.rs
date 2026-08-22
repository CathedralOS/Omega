//! Ordered affine-witness candidates for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

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
            definition_words.iter().find_map(|definition_axioms| {
                let literal_axioms = frontier::literal_axioms(
                    context,
                    semantic_axioms,
                    root,
                    definition_axioms,
                    target,
                )?;
                complete(IntegerAffineWitness {
                    root: root.clone(),
                    target: target.clone(),
                    literal_axioms,
                    definition_axioms: definition_axioms.clone(),
                })
            })
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
    definition_words.iter().find_map(|definition_axioms| {
        let literal_axioms =
            frontier::literal_axioms(context, semantic_axioms, root, definition_axioms, target)?;
        complete(IntegerAffineWitness {
            root: root.clone(),
            target: target.clone(),
            literal_axioms,
            definition_axioms: definition_axioms.clone(),
        })
    })
}
