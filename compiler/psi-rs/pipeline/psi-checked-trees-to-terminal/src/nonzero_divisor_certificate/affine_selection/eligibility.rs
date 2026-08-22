//! Exact affine root and alias eligibility for certificate production.

use psi_core::{Proposition, ScalarTerm, ScalarType};

pub(super) fn ordered_value_endpoints<'a>(
    left: &'a ScalarTerm,
    right: &'a ScalarTerm,
) -> impl Iterator<Item = &'a ScalarTerm> {
    [left, right]
        .into_iter()
        .filter(|endpoint| matches!(endpoint, ScalarTerm::Value { .. }))
}

pub(super) fn distinct_value_alias(root: &ScalarTerm, alias: &ScalarTerm) -> bool {
    root != alias
        && matches!(root, ScalarTerm::Value { .. })
        && matches!(alias, ScalarTerm::Value { .. })
}

pub(super) fn exact_value_binding(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    matches!(root, ScalarTerm::Value { .. })
        && literal.integer_value().is_some_and(|(integer_type, _)| {
            root.scalar_type() == ScalarType::Integer(integer_type)
        })
}

pub(super) fn one_alias_join(
    outer_equality: &Proposition,
    root: &ScalarTerm,
    inner_equality: &Proposition,
    literal: &ScalarTerm,
) -> bool {
    !std::ptr::eq(outer_equality, inner_equality) && exact_value_binding(root, literal)
}
