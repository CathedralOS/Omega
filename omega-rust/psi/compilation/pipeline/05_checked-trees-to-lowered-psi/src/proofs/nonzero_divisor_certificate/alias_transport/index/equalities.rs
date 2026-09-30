//! Citation-preserving endpoint index for alias-equality production.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};
use super::super::super::scope_identity::{self, ScopeDerived};

/// Cited `Equal` facts keyed by either endpoint: each entry carries the fact's
/// other endpoint, so a lookup by one side yields the candidate bound aliases
/// in source order without rescanning every fact.
pub(in super::super) struct EqualityIndex<'a> {
    /// Per endpoint, each incident equality in citation order and whether its
    /// other endpoint is the left side.
    by_endpoint: Rc<BTreeMap<ScalarTerm, Vec<(Citation, bool)>>>,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> EqualityIndex<'a> {
    pub(in super::super) fn get(
        &self,
        endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm)> + '_ {
        self.by_endpoint
            .get(endpoint)
            .into_iter()
            .flatten()
            .map(|&(citation, other_is_left)| {
                let fact = citation.fact(self.assumptions, self.semantic_axioms);
                let Proposition::Equal(left, right) = fact else {
                    unreachable!("the equality index holds only cited equalities")
                };
                (citation, fact, if other_is_left { left } else { right })
            })
    }
}

/// The endpoint index is a pure function of the scope, so it is derived once
/// per scope and resolved against the caller's slices.
pub(in super::super) fn indexed_equalities<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> EqualityIndex<'a> {
    thread_local! {
        static INDEXES: RefCell<ScopeDerived<BTreeMap<ScalarTerm, Vec<(Citation, bool)>>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    let by_endpoint = scope_identity::derived(&INDEXES, assumptions, semantic_axioms, || {
        let mut aliases_by_endpoint = BTreeMap::<_, Vec<_>>::new();
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::Equal(left, right) = fact else {
                continue;
            };
            aliases_by_endpoint
                .entry(left.clone())
                .or_default()
                .push((citation, false));
            if right != left {
                aliases_by_endpoint
                    .entry(right.clone())
                    .or_default()
                    .push((citation, true));
            }
        }
        aliases_by_endpoint
    });
    EqualityIndex {
        by_endpoint,
        assumptions,
        semantic_axioms,
    }
}
