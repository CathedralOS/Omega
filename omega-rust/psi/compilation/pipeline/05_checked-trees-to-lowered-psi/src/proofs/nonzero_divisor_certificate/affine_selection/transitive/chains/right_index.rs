//! Source-ordered right-leg index for affine certificate production.

use std::cell::RefCell;
use std::rc::Rc;

use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::Citation;
use super::super::super::super::scope_identity::{self, ScopeDerived};
use super::super::super::{bounds, value_index::ValueIndex};

pub(super) struct RightLegIndex<'a> {
    /// Value-left cited bounds by their left endpoint, as citations: a pure
    /// function of the scope, derived once per scope.
    by_left_endpoint: Rc<ValueIndex<Citation>>,
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
}

impl<'a> RightLegIndex<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        thread_local! {
            static INDEXES: RefCell<ScopeDerived<ValueIndex<Citation>>> =
                const { RefCell::new(ScopeDerived::new()) };
        }
        let by_left_endpoint =
            scope_identity::derived(&INDEXES, assumptions, semantic_axioms, || {
                let mut by_left_endpoint = ValueIndex::new();
                for (citation, _, left, _) in bounds::with_value_left(assumptions, semantic_axioms)
                {
                    by_left_endpoint.push(left, citation);
                }
                by_left_endpoint
            });
        Self {
            by_left_endpoint,
            assumptions,
            semantic_axioms,
        }
    }

    pub(super) fn candidates(
        &self,
        left_endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm)> + '_ {
        self.by_left_endpoint
            .candidates(left_endpoint)
            .iter()
            .map(|&citation| {
                let fact = citation.fact(self.assumptions, self.semantic_axioms);
                let Proposition::LessOrEqual(_, right) = fact else {
                    unreachable!("right legs are cited order bounds")
                };
                (citation, fact, right)
            })
    }
}
