//! Ordered affine-witness candidates for certificate production.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::{definition_index::DefinitionIndex, frontier};

pub(super) fn find<T>(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    definitions: &DefinitionIndex,
    root: &ScalarTerm,
    mut complete: impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for target in [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
    {
        for definition_axioms in
            frontier::definition_words(context, semantic_axioms, definitions, root)
        {
            if let Some(result) = complete(IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
            }) {
                return Some(result);
            }
        }
    }
    None
}
