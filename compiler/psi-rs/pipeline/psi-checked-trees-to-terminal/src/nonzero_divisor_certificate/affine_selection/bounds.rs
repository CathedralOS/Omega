//! Source-ordered retained integer bounds for certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::integer_evidence::{Citation, cited_facts};

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
        .filter(|endpoint| matches!(endpoint, ScalarTerm::Value { .. }))
}
