//! Exact affine root and alias eligibility for certificate production.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn is_value(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. })
}

pub(super) fn distinct_facts(left: &Proposition, right: &Proposition) -> bool {
    !std::ptr::eq(left, right)
}

pub(super) fn ordered_value_endpoints<'a>(
    left: &'a ScalarTerm,
    right: &'a ScalarTerm,
) -> impl Iterator<Item = &'a ScalarTerm> {
    [left, right]
        .into_iter()
        .filter(|endpoint| is_value(endpoint))
}

pub(super) fn distinct_value_alias(root: &ScalarTerm, alias: &ScalarTerm) -> bool {
    root != alias && is_value(root) && is_value(alias)
}
