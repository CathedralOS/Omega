//! Exact affine root and alias eligibility for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn is_value(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. })
}

pub(super) fn distinct_facts(left: &Proposition, right: &Proposition) -> bool {
    !std::ptr::eq(left, right)
}
