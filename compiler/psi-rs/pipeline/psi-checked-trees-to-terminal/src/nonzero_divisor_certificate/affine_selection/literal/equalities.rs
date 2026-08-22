//! Source-ordered oriented equalities for affine-literal certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct OrientedEqualities<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> OrientedEqualities<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
        }
    }

    pub(super) fn iter(
        &self,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm)> + '_
    {
        cited_facts(self.assumptions, self.semantic_axioms)
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
}
