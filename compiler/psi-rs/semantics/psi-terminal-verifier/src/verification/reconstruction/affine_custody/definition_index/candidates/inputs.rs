//! Affine-definition input projection for independent reconstruction.

use psi_core::ScalarTerm;

pub(super) fn affine(expression: &ScalarTerm) -> [Option<&ScalarTerm>; 2] {
    match expression {
        ScalarTerm::ExactIntegerAdd { left, right, .. }
        | ScalarTerm::ExactIntegerMultiply { left, right, .. } => {
            [Some(left.as_ref()), Some(right.as_ref())]
        }
        ScalarTerm::ExactIntegerSubtract { left, .. } => [Some(left.as_ref()), None],
        _ => [None, None],
    }
}
