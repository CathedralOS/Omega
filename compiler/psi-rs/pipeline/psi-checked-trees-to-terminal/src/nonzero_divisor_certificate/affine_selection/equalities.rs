//! Source-ordered oriented equalities for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::integer_evidence::{Citation, cited_facts};
use super::eligibility;

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
        .filter(|(_, _, root, literal)| eligibility::exact_value_binding(root, literal))
}
