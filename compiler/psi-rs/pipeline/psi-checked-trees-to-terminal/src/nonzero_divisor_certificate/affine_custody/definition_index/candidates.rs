//! Producer-local syntactic affine-definition candidates.

use psi_core::{Proposition, ScalarTerm};

mod inputs;
mod orientations;

pub(super) fn visit(
    semantic_axioms: &[Proposition],
    mut candidate: impl FnMut(usize, &ScalarTerm),
) {
    for (index, proposition) in semantic_axioms.iter().enumerate() {
        for expression in orientations::value_target_expressions(proposition) {
            for input in inputs::affine(expression).into_iter().flatten() {
                if matches!(input, ScalarTerm::Value { .. }) {
                    candidate(index, input);
                }
            }
        }
    }
}
