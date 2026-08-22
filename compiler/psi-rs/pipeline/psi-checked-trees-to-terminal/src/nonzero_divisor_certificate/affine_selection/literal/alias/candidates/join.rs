//! Producer-local exact root-alias/alias-literal join eligibility.

use psi_core::{Proposition, ScalarTerm, ScalarType};

pub(super) fn eligible(
    outer_equality: &Proposition,
    root: &ScalarTerm,
    inner_equality: &Proposition,
    literal: &ScalarTerm,
) -> bool {
    if std::ptr::eq(outer_equality, inner_equality) {
        return false;
    }
    let Some((integer_type, _)) = literal.integer_value() else {
        unreachable!("literal index contains only integer landings")
    };
    root.scalar_type() == ScalarType::Integer(integer_type)
}
