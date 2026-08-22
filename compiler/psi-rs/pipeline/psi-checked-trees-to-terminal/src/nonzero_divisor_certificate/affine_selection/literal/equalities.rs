//! Source-ordered oriented equalities for affine-literal certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

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
