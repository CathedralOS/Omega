//! Ordered affine-witness candidates for independent reconstruction.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

pub(super) fn any(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> bool,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .any(|target| {
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
