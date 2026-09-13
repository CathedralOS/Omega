//! Source-ordered oriented equalities for affine certificate production.

use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::super::integer_evidence::{Citation, cited_facts};

fn exact_value_binding(root: &ScalarTerm, literal: &ScalarTerm) -> bool {
    matches!(root, ScalarTerm::Value { .. })
        && literal.integer_value().is_some_and(|(integer_type, _)| {
            root.scalar_type() == ScalarType::Integer(integer_type)
        })
}

pub(super) fn ordered<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    cited_facts(assumptions, semantic_axioms)
        .filter_map(|(citation, equality)| match equality {
            Proposition::Equal(left, right) => Some((citation, equality, left, right)),
            _ => None,
        })
        .flat_map(|(citation, equality, left, right)| {
            [
                (citation, equality, left, right),
                (citation, equality, right, left),
            ]
        })
}

pub(super) fn exact_value_bindings<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(assumptions, semantic_axioms)
        .filter(|(_, _, root, literal)| exact_value_binding(root, literal))
}

pub(super) fn value_aliases<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(assumptions, semantic_axioms).filter(|(_, _, root, alias)| {
        root != alias
            && matches!(root, ScalarTerm::Value { .. })
            && matches!(alias, ScalarTerm::Value { .. })
    })
}
