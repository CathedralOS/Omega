//! Producer-local exact root-alias/alias-literal join eligibility.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::eligibility;

pub(super) fn eligible(
    outer_equality: &Proposition,
    root: &ScalarTerm,
    inner_equality: &Proposition,
    literal: &ScalarTerm,
) -> bool {
    if std::ptr::eq(outer_equality, inner_equality) {
        return false;
    }
    eligibility::exact_value_binding(root, literal)
}
