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
    targets::values(goal).any(|target| {
        frontier::definition_words(context, semantic_axioms, definitions, root)
            .into_iter()
            .any(|definition_axioms| {
                complete(IntegerAffineWitness {
                    root: root.clone(),
                    target: target.clone(),
                    definition_axioms,
                })
            })
    })
}
