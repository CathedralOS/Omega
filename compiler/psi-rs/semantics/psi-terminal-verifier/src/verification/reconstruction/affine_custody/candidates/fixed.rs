//! Verifier-local completion of affine witnesses for one exact target.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::IntegerAffineWitness;

use super::super::frontier;

pub(super) fn any(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_words: &[Vec<usize>],
    root: &ScalarTerm,
    target: &ScalarTerm,
    complete: &mut impl FnMut(IntegerAffineWitness) -> bool,
) -> bool {
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
}

pub(super) fn find<T>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    definition_words: &[Vec<usize>],
    root: &ScalarTerm,
    target: &ScalarTerm,
    complete: &mut impl FnMut(IntegerAffineWitness) -> Option<T>,
) -> Option<T> {
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
