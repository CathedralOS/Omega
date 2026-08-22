//! Independent direct two-citation transitive root bound.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn retained(left: &ScalarTerm, right: &ScalarTerm) -> Proposition {
    Proposition::LessOrEqual(left.clone(), right.clone())
}
