//! Source-ordered retained integer bounds for certificate production.

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::integer_evidence::{Citation, cited_facts};

fn is_value(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. })
}

pub(super) fn with_value_left<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(assumptions, semantic_axioms).filter(|(_, _, left, _)| is_value(left))
}

pub(super) fn with_value_right<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    ordered(assumptions, semantic_axioms).filter(|(_, _, _, right)| is_value(right))
}

pub(super) fn ordered<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + 'a {
    cited_facts(assumptions, semantic_axioms).filter_map(|(citation, fact)| match fact {
        Proposition::LessOrEqual(left, right) => Some((citation, fact, left, right)),
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
