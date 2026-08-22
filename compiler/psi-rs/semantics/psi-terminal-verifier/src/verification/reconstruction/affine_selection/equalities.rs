//! Source-ordered oriented equalities for independent affine reconstruction.

use psi_core::{Proposition, ScalarTerm, ScalarType};

fn exact_value_binding(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    matches!(root, ScalarTerm::Value { .. })
        && literal.integer_value().is_some_and(|(integer_type, _)| {
            root.scalar_type() == ScalarType::Integer(integer_type)
        })
}

pub(super) fn ordered<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((equality, left, right)),
            _ => None,
        })
        .flat_map(|(equality, left, right)| [(equality, left, right), (equality, right, left)])
}

pub(super) fn exact_value_bindings<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(requirements, semantic_axioms)
        .filter(|(_, root, literal)| exact_value_binding(root, literal))
}

pub(super) fn value_aliases<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(requirements, semantic_axioms).filter(|(_, root, alias)| {
        root != alias
            && matches!(root, ScalarTerm::Value { .. })
            && matches!(alias, ScalarTerm::Value { .. })
    })
}
