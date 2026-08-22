//! Ordered exact two-citation chains for independent affine reconstruction.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

pub(super) struct TwoCitationChains<'a> {
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    bounds_by_left_endpoint: BTreeMap<ScalarTerm, Vec<&'a Proposition>>,
}

impl<'a> TwoCitationChains<'a> {
    pub(super) fn new(requirements: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        let mut bounds_by_left_endpoint = BTreeMap::<_, Vec<_>>::new();
        for fact in requirements.iter().chain(semantic_axioms) {
            let Proposition::LessOrEqual(left, _) = fact else {
                continue;
            };
            if matches!(left, ScalarTerm::Value { .. }) {
                bounds_by_left_endpoint
                    .entry(left.clone())
                    .or_default()
                    .push(fact);
            }
        }
        Self {
            requirements,
            semantic_axioms,
            bounds_by_left_endpoint,
        }
    }

    pub(super) fn any(
        &self,
        mut complete: impl FnMut(&'a Proposition, &'a Proposition) -> bool,
    ) -> bool {
        for left_fact in self.requirements.iter().chain(self.semantic_axioms) {
            let Proposition::LessOrEqual(_, middle) = left_fact else {
                continue;
            };
            if !matches!(middle, ScalarTerm::Value { .. }) {
                continue;
            }
            let Some(right_facts) = self.bounds_by_left_endpoint.get(middle) else {
                continue;
            };
            for &right_fact in right_facts {
                if std::ptr::eq(left_fact, right_fact) {
                    continue;
                }
                if complete(left_fact, right_fact) {
                    return true;
                }
            }
        }
        false
    }
}
