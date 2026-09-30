//! Citation-preserving endpoint index for alias-order transport.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{Citation, cited_facts};
use super::super::super::scope_identity::{self, ScopeDerived};

/// Cited `LessOrEqual` facts keyed by either endpoint: each entry carries the
/// fact's other endpoint and the key's endpoint position, so a lookup by one
/// side yields the candidate literal endpoints in source order without
/// rescanning every fact.
pub(in super::super) struct RelationIndex<'a> {
    by_endpoint: Rc<BTreeMap<ScalarTerm, Vec<(Citation, usize)>>>,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> RelationIndex<'a> {
    pub(in super::super) fn get(
        &self,
        endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, usize)> + '_ {
        self.by_endpoint
            .get(endpoint)
            .into_iter()
            .flatten()
            .map(|&(citation, position)| {
                let fact = citation.fact(self.assumptions, self.semantic_axioms);
                let Proposition::LessOrEqual(left, right) = fact else {
                    unreachable!("the relation index holds only cited order bounds")
                };
                (
                    citation,
                    fact,
                    if position == 0 { right } else { left },
                    position,
                )
            })
    }
}

/// Derived once per scope; see `indexed_equalities`.
pub(in super::super) fn indexed_relations<'a>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> RelationIndex<'a> {
    thread_local! {
        static INDEXES: RefCell<ScopeDerived<BTreeMap<ScalarTerm, Vec<(Citation, usize)>>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    let by_endpoint = scope_identity::derived(&INDEXES, assumptions, semantic_axioms, || {
        let mut relations_by_endpoint = BTreeMap::<_, Vec<_>>::new();
        for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(left, right) = fact else {
                continue;
            };
            relations_by_endpoint
                .entry(left.clone())
                .or_default()
                .push((citation, 0));
            if right != left {
                relations_by_endpoint
                    .entry(right.clone())
                    .or_default()
                    .push((citation, 1));
            }
        }
        relations_by_endpoint
    });
    RelationIndex {
        by_endpoint,
        assumptions,
        semantic_axioms,
    }
}
