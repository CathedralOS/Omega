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
    targets::values(goal).find_map(|target| {
        frontier::definition_words(context, semantic_axioms, definitions, root)
            .into_iter()
            .find_map(|definition_axioms| {
                complete(IntegerAffineWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms,
                })
            })
    })
}
