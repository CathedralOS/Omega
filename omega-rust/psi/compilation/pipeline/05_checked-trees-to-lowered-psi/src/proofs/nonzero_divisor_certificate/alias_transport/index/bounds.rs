//! Citation-preserving endpoint index for alias-bound production.

use std::collections::BTreeMap;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

pub(in super::super) fn indexed_bounds<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, usize)>> {
    let mut bounds_by_endpoint = BTreeMap::<_, Vec<_>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        if matches!(left, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(left.clone())
                .or_default()
                .push((citation, fact, 0));
        }
        if right != left && matches!(right, ScalarTerm::Value { .. }) {
            bounds_by_endpoint
                .entry(right.clone())
                .or_default()
                .push((citation, fact, 1));
        }
    }
    bounds_by_endpoint
}
