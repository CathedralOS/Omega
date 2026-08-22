//! Source-ordered left legs for affine certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct LeftLegs<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> LeftLegs<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
        }
    }

    pub(super) fn find<T>(
        &self,
        mut join: impl FnMut(Citation, &'a Proposition, &'a ScalarTerm) -> Option<T>,
    ) -> Option<T> {
        for (citation, fact) in cited_facts(self.assumptions, self.semantic_axioms) {
            let Proposition::LessOrEqual(_, middle) = fact else {
                continue;
            };
            if !matches!(middle, ScalarTerm::Value { .. }) {
                continue;
            }
            if let Some(result) = join(citation, fact, middle) {
                return Some(result);
            }
        }
        None
    }
}
