//! Ordered affine-witness candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

mod targets;

pub(super) fn any(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> bool,
) -> bool {
    let mut targets = targets::values(goal);
    let Some(first_target) = targets.next() else {
        return false;
    };
    let definition_words = frontier::definition_words(context, semantic_axioms, definitions, root);
    std::iter::once(first_target).chain(targets).any(|target| {
        definition_words.iter().any(|definition_axioms| {
            let Some(literal_axioms) =
                frontier::literal_axioms(context, semantic_axioms, root, definition_axioms, target)
            else {
                return false;
            };
            complete(IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                literal_axioms,
                definition_axioms: definition_axioms.clone(),
            })
        })
    })
}
