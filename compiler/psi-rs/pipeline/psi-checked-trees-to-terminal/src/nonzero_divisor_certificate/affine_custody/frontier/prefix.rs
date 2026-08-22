//! Producer-local replay of one affine-definition frontier prefix.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{IntegerAffineWitness, check_integer_affine_witness};

pub(super) fn checked_target<'a>(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    definition_axioms: &[usize],
    definition: &'a Proposition,
) -> Option<&'a ScalarTerm> {
    let Proposition::Equal(left, right) = definition else {
        unreachable!("definition index contains only equality rows")
    };
    [left, right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .find(|target| {
            check_integer_affine_witness(
                context,
                semantic_axioms,
                &IntegerAffineWitness {
                    root: root.clone(),
                    target: (*target).clone(),
                    definition_axioms: definition_axioms.to_vec(),
                },
            )
            .is_ok()
        })
}
