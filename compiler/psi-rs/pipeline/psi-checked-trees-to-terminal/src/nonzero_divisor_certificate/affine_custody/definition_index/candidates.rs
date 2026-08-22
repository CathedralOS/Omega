//! Producer-local syntactic affine-definition candidates.

use psi_core::{Proposition, ScalarTerm};

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
            for input in affine_inputs(expression).into_iter().flatten() {
                if matches!(input, ScalarTerm::Value { .. }) {
                    candidate(index, input);
                }
            }
        }
    }
}

fn affine_inputs(expression: &ScalarTerm) -> [Option<&ScalarTerm>; 2] {
    match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        ScalarTerm::ExactIntegerSubtract { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    }
}
