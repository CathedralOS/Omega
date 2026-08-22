//! Exact affine root and alias eligibility for independent reconstruction.

use psi_core::{IntegerType, Proposition, ScalarTerm, ScalarType};

pub(super) fn is_value(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. })
}

pub(super) fn integer_literal_type(term: &ScalarTerm) -> Option<IntegerType> {
    term.integer_value().map(|(integer_type, _)| integer_type)
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

pub(super) fn exact_value_binding(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    is_value(root)
        && integer_literal_type(literal)
            .is_some_and(|integer_type| root.scalar_type() == ScalarType::Integer(integer_type))
}

pub(super) fn one_alias_join(
    outer_equality: &Proposition,
    root: &ScalarTerm,
    inner_equality: &Proposition,
    literal: &ScalarTerm,
) -> bool {
    distinct_facts(outer_equality, inner_equality) && exact_value_binding(root, literal)
}
