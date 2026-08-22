//! Independent replay of one affine-definition frontier prefix.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerAffineWitness, check_integer_affine_witness};

mod literals;
mod targets;

pub(super) fn literal_axioms(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    definition_axioms: &[usize],
    target: &ScalarTerm,
) -> Option<Vec<Option<usize>>> {
    literals::select(context, semantic_axioms, root, definition_axioms, target)
}

pub(super) fn checked_target<'a>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    definition_axioms: &[usize],
    definition: &'a Proposition,
) -> Option<&'a ScalarTerm> {
    targets::values(definition).find(|target| {
        let Some(literal_axioms) =
            literal_axioms(context, semantic_axioms, root, definition_axioms, target)
        else {
            return false;
        };
        check_integer_affine_witness(
            context,
            semantic_axioms,
            &IntegerAffineWitness {
                root: root.clone(),
                target: (*target).clone(),
                literal_axioms,
                definition_axioms: definition_axioms.to_vec(),
            },
        )
        .is_ok()
    })
}
