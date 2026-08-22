//! Syntactic affine-definition candidates for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

mod inputs;

pub(super) fn visit(
    semantic_axioms: &[Proposition],
    mut candidate: impl FnMut(usize, &ScalarTerm),
) {
    for (index, proposition) in semantic_axioms.iter().enumerate() {
        let Proposition::Equal(left, right) = proposition else {
            continue;
        };
        for (target, expression) in [(left, right), (right, left)] {
            if !matches!(target, ScalarTerm::Value { .. }) {
                continue;
            }
            for input in inputs::affine(expression).into_iter().flatten() {
                if matches!(input, ScalarTerm::Value { .. }) {
                    candidate(index, input);
                }
            }
        }
    }
}
