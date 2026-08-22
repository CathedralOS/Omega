//! Exact affine root/integer-literal eligibility for certificate production.

use psi_core::{ScalarTerm, ScalarType};

pub(super) fn exact_value_binding(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    matches!(root, ScalarTerm::Value { .. })
        && literal.integer_value().is_some_and(|(integer_type, _)| {
            root.scalar_type() == ScalarType::Integer(integer_type)
        })
}
