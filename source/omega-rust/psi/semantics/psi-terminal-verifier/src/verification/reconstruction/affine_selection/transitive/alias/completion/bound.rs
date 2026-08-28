//! Independent alias-substituted transitive affine root bound.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn retained(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    left: &ScalarTerm,
    right: &ScalarTerm,
) -> Option<Proposition> {
    if alias == left {
        Some(Proposition::LessOrEqual(root.clone(), right.clone()))
    } else if alias == right {
        Some(Proposition::LessOrEqual(left.clone(), root.clone()))
    } else {
        None
    }
}
