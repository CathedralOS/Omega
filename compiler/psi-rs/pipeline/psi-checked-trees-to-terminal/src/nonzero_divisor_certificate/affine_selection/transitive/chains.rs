//! Ordered exact two-citation chains for affine certificate production.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

pub(super) struct TwoCitationChains<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    bounds_by_left_endpoint: BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition)>>,
}

impl<'a> TwoCitationChains<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut bounds_by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(left, _) = fact else {
                continue;
            };
            if matches!(left, ScalarTerm::Value { .. }) {
                bounds_by_left_endpoint
                    .entry(left.clone())
                    .or_default()
                    .push((citation, fact));
            }
        }
        Self {
            assumptions,
            semantic_axioms,
            bounds_by_left_endpoint,
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(Citation, &'a Proposition, Citation, &'a Proposition) -> Option<T>,
    ) -> Option<T> {
        for (left_citation, left_fact) in cited_facts(self.assumptions, self.semantic_axioms) {
            let Proposition::LessOrEqual(_, middle) = left_fact else {
                continue;
            };
            if !matches!(middle, ScalarTerm::Value { .. }) {
                continue;
            }
            let Some(right_facts) = self.bounds_by_left_endpoint.get(middle) else {
                continue;
            };
            for &(right_citation, right_fact) in right_facts {
                if std::ptr::eq(left_fact, right_fact) {
                    continue;
                }
                if let Some(result) = complete(left_citation, left_fact, right_citation, right_fact)
                {
                    return Some(result);
                }
            }
        }
        None
    }
}
