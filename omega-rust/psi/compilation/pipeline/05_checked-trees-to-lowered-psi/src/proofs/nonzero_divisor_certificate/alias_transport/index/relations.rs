//! Citation-preserving endpoint index for alias-order transport.

use std::collections::BTreeMap;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

/// Cited `LessOrEqual` facts keyed by either endpoint: each entry carries the
/// fact's other endpoint and the key's endpoint position, so a lookup by one
/// side yields the candidate literal endpoints in source order without
/// rescanning every fact.
pub(in super::super) fn indexed_relations<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, &'a ScalarTerm, usize)>> {
    let mut relations_by_endpoint = BTreeMap::<_, Vec<_>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, right) = fact else {
            continue;
        };
        relations_by_endpoint
            .entry(left.clone())
            .or_default()
            .push((citation, fact, right, 0));
        if right != left {
            relations_by_endpoint
                .entry(right.clone())
                .or_default()
                .push((citation, fact, left, 1));
        }
    }
    relations_by_endpoint
}
