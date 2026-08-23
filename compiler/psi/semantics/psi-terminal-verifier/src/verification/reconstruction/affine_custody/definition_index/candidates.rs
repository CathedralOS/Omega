//! Syntactic affine-definition candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod inputs;
mod orientations;

pub(super) fn visit(
    semantic_axioms: &[Proposition],
    mut candidate: impl FnMut(usize, &ScalarTerm),
) {
    for (index, proposition) in semantic_axioms.iter().enumerate() {
        for expression in orientations::value_target_expressions(proposition) {
            for input in inputs::affine_values(expression) {
                candidate(index, input);
            }
        }
    }
}
