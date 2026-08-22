//! Source-ordered retained integer bounds for independent reconstruction.

use psi_core::{Proposition, ScalarTerm};

fn is_value(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. })
}

pub(super) fn with_value_left<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(requirements, semantic_axioms).filter(|(_, left, _)| is_value(left))
}

pub(super) fn with_value_right<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(requirements, semantic_axioms).filter(|(_, _, right)| is_value(right))
}

pub(super) fn ordered<'a>(
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (&'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|fact| match fact {
            Proposition::LessOrEqual(left, right) => Some((fact, left, right)),
            _ => None,
        })
}

pub(super) fn value_endpoints<'a>(
    left: &'a ScalarTerm,
    right: &'a ScalarTerm,
) -> impl Iterator<Item = &'a ScalarTerm> {
    [left, right]
        .into_iter()
        .filter(|endpoint| is_value(endpoint))
}
