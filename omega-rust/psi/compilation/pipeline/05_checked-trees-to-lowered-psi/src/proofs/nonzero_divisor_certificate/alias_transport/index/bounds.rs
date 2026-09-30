//! Citation-preserving endpoint index for alias-bound production.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};
use super::super::super::scope_identity::{self, ScopeDerived};

/// Cited value-endpoint `LessOrEqual` facts keyed by that endpoint, each with
/// the endpoint's position in the relation.
pub(in super::super) struct BoundIndex<'a> {
    by_endpoint: Rc<BTreeMap<ScalarTerm, Vec<(Citation, usize)>>>,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> BoundIndex<'a> {
    pub(in super::super) fn get(
        &self,
        endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, usize)> + '_ {
        self.by_endpoint
            .get(endpoint)
            .into_iter()
            .flatten()
            .map(|&(citation, position)| {
                (
                    citation,
                    citation.fact(self.assumptions, self.semantic_axioms),
                    position,
                )
            })
    }
}

/// Derived once per scope; see `indexed_equalities`.
pub(in super::super) fn indexed_bounds<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BoundIndex<'a> {
    thread_local! {
        static INDEXES: RefCell<ScopeDerived<BTreeMap<ScalarTerm, Vec<(Citation, usize)>>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    let by_endpoint = scope_identity::derived(&INDEXES, assumptions, semantic_axioms, || {
        let mut bounds_by_endpoint = BTreeMap::<_, Vec<_>>::new();
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(left, right) = fact else {
                continue;
            };
            if matches!(left, ScalarTerm::Value { .. }) {
                bounds_by_endpoint
                    .entry(left.clone())
                    .or_default()
                    .push((citation, 0));
            }
            if right != left && matches!(right, ScalarTerm::Value { .. }) {
                bounds_by_endpoint
                    .entry(right.clone())
                    .or_default()
                    .push((citation, 1));
            }
        }
        bounds_by_endpoint
    });
    BoundIndex {
        by_endpoint,
        assumptions,
        semantic_axioms,
    }
}
