//! Citation-preserving endpoint index for alias-equality production.

use std::collections::BTreeMap;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};

/// Cited `Equal` facts keyed by either endpoint: each entry carries the fact's
/// other endpoint, so a lookup by one side yields the candidate bound aliases
/// in source order without rescanning every fact.
pub(in super::super) fn indexed_equalities<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> BTreeMap<ScalarTerm, Vec<(Citation, &'a Proposition, &'a ScalarTerm)>> {
    let mut aliases_by_endpoint = BTreeMap::<_, Vec<_>>::new();
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(left, right) = fact else {
            continue;
        };
        aliases_by_endpoint
            .entry(left.clone())
            .or_default()
            .push((citation, fact, right));
        if right != left {
            aliases_by_endpoint
                .entry(right.clone())
                .or_default()
                .push((citation, fact, left));
        }
    }
    aliases_by_endpoint
}
