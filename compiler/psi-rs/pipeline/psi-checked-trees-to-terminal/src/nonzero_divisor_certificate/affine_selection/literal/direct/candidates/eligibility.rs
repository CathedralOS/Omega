//! Producer-local exact direct root/literal eligibility.

use psi_core::{ScalarTerm, ScalarType};

pub(super) fn eligible(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    matches!(root, ScalarTerm::Value { .. })
        && literal.integer_value().is_some_and(|(integer_type, _)| {
            root.scalar_type() == ScalarType::Integer(integer_type)
        })
}
