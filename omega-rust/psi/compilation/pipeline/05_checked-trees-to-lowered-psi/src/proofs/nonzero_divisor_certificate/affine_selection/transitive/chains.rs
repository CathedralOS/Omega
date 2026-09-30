//! Ordered exact two-citation chains for affine certificate production.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::Citation;
use super::super::super::scope_identity::{self, ScopeDerived};
use super::super::fact_identity;

mod left_legs;
mod right_index;

use right_index::RightLegIndex;

pub(super) struct TwoCitationChains<'a> {
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    right_legs: RightLegIndex<'a>,
}

impl<'a> TwoCitationChains<'a> {
    pub(super) fn new(assumptions: &'a [Proposition], semantic_axioms: &'a [Proposition]) -> Self {
        Self {
            assumptions,
            semantic_axioms,
            right_legs: RightLegIndex::new(assumptions, semantic_axioms),
        }
    }

    pub(super) fn find<T>(
        &self,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode, ProofNode) -> Option<T>,
    ) -> Option<T> {
        left_legs::find(
            self.assumptions,
            self.semantic_axioms,
            |left_citation, left_fact, left, middle| {
                self.right_legs.candidates(middle).find_map(
                    |(right_citation, right_fact, right)| {
                        if !fact_identity::distinct(left_fact, right_fact) {
                            return None;
                        }
                        complete(
                            left,
                            right,
                            left_citation.proof(left_fact),
                            right_citation.proof(right_fact),
                        )
                    },
                )
            },
        )
    }

    /// `find` over only the chains that name `endpoint` as an outer
    /// endpoint, in the same order `find` meets them. A completion that
    /// substitutes at an outer endpoint refuses every other chain from its
    /// endpoints alone, so those chains are never walked or cited: the
    /// scope's chains are indexed once by outer endpoint.
    pub(super) fn find_touching<T>(
        &self,
        endpoint: &ScalarTerm,
        mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode, ProofNode) -> Option<T>,
    ) -> Option<T> {
        let index = self.by_outer_endpoint();
        index
            .get(endpoint)
            .into_iter()
            .flatten()
            .find_map(|&(left_citation, right_citation)| {
                let left_fact = left_citation.fact(self.assumptions, self.semantic_axioms);
                let right_fact = right_citation.fact(self.assumptions, self.semantic_axioms);
                let (Proposition::LessOrEqual(left, _), Proposition::LessOrEqual(_, right)) =
                    (left_fact, right_fact)
                else {
                    unreachable!("chain legs are cited order bounds")
                };
                complete(
                    left,
                    right,
                    left_citation.proof(left_fact),
                    right_citation.proof(right_fact),
                )
            })
    }

    /// Every chain `find` walks, as `(left leg, right leg)` citations in walk
    /// order, listed under each distinct outer endpoint it names. A pure
    /// function of the scope, derived once per scope.
    fn by_outer_endpoint(&self) -> Rc<BTreeMap<ScalarTerm, Vec<(Citation, Citation)>>> {
        thread_local! {
            static INDEXES: RefCell<
                ScopeDerived<BTreeMap<ScalarTerm, Vec<(Citation, Citation)>>>,
            > = const { RefCell::new(ScopeDerived::new()) };
        }
        scope_identity::derived(&INDEXES, self.assumptions, self.semantic_axioms, || {
            let mut index = BTreeMap::<ScalarTerm, Vec<(Citation, Citation)>>::new();
            left_legs::find(
                self.assumptions,
                self.semantic_axioms,
                |left_citation, left_fact, left, middle| {
                    for (right_citation, right_fact, right) in self.right_legs.candidates(middle) {
                        if !fact_identity::distinct(left_fact, right_fact) {
                            continue;
                        }
                        let chain = (left_citation, right_citation);
                        index.entry(left.clone()).or_default().push(chain);
                        if right != left {
                            index.entry(right.clone()).or_default().push(chain);
                        }
                    }
                    None::<()>
                },
            );
            index
        })
    }
}
