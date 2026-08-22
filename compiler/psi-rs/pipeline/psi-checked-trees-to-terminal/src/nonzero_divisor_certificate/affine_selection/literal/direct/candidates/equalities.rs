//! Source-ordered oriented equalities for direct literal certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::super::integer_evidence::{Citation, cited_facts};

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

    pub(super) fn find<T>(
        &self,
        mut candidate: impl FnMut(
            Citation,
            &'a Proposition,
            &'a ScalarTerm,
            &'a ScalarTerm,
        ) -> Option<T>,
    ) -> Option<T> {
        for (citation, equality) in cited_facts(self.assumptions, self.semantic_axioms) {
            let Proposition::Equal(left, right) = equality else {
                continue;
            };
            for (root, literal) in [(left, right), (right, left)] {
                if let Some(result) = candidate(citation, equality, root, literal) {
                    return Some(result);
                }
            }
        }
        None
    }
}
