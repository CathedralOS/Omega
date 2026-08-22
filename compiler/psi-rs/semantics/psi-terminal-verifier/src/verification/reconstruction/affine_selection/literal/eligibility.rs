//! Exact affine root/integer-literal eligibility for independent reconstruction.

use psi_core::{Proposition, ScalarTerm, ScalarType};

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
