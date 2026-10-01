//! Root sources built from cited order facts: one retained bound, or two
//! retained bounds joined by transitivity.
//!
//! The two-citation chain is fixed at exactly two citations and at most one
//! alias equality. It proposes candidates directly to the completion rather
//! than recursing through the general integer-bound search, so neither
//! equality chains nor longer order paths are admitted here.

use std::cell::RefCell;
use std::rc::Rc;

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::integer_evidence::Citation;
use super::super::scope_identity::{self, ScopeDerived};
use super::roster::{
    ValueIndex, cited_bounds, is_value, oriented_equality, substitute_subject, value_aliases,
    value_endpoints,
};
use super::{Complete, Request};

pub(super) fn retained_bound(request: Request, complete: &mut dyn Complete) -> Option<ProofNode> {
    cited_bounds(request).find_map(|(citation, fact, left, right)| {
        value_endpoints(left, right).find_map(|root| {
            if !complete.admits(root) {
                return None;
            }
            complete.complete(root, citation.proof(fact))
        })
    })
}

pub(super) fn two_citation_chain(
    request: Request,
    complete: &mut dyn Complete,
) -> Option<ProofNode> {
    TwoCitationChains::new(request).find(|left, right, left_proof, right_proof| {
        if !value_endpoints(left, right).any(|root| complete.admits(root)) {
            return None;
        }
        let root_bound = transitive(left, right, left_proof, right_proof);
        value_endpoints(left, right).find_map(|root| {
            if !complete.admits(root) {
                return None;
            }
            complete.complete(root, root_bound.clone())
        })
    })
}

/// A chain `left <= right` whose `alias` endpoint is renamed to `root` through
/// one cited equality `root == alias`.
pub(super) fn two_citation_chain_through_alias(
    request: Request,
    complete: &mut dyn Complete,
) -> Option<ProofNode> {
    let chains = TwoCitationChains::new(request);
    // The substitution renames the alias at one chain endpoint, so only the
    // value-alias readings whose alias some chain names at either end can
    // complete; every goal under the scope walks the same readings, so they
    // are kept once per scope.
    thread_local! {
        static READINGS: RefCell<ScopeDerived<Vec<(Citation, bool)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    let readings = scope_identity::derived(
        &READINGS,
        request.assumptions,
        request.semantic_axioms,
        || {
            value_aliases(request)
                .filter(|(_, _, _, alias)| chains.touches(alias))
                .map(|(citation, equality, _, alias)| {
                    let Proposition::Equal(left, _) = equality else {
                        unreachable!("value aliases read cited equalities")
                    };
                    (citation, std::ptr::eq(left, alias))
                })
                .collect()
        },
    );
    readings.iter().find_map(|&(citation, reversed)| {
        let (equality_citation, equality, root, alias) =
            oriented_equality(request, citation, reversed);
        if !complete.admits(root) {
            return None;
        }
        chains.find_touching(alias, |left, right, left_proof, right_proof| {
            let (endpoint, bound) = if alias == left {
                (0, right)
            } else if alias == right {
                (1, left)
            } else {
                return None;
            };
            let root_bound = substitute_subject(
                transitive(left, right, left_proof, right_proof),
                bound,
                endpoint,
                [(root, equality_citation.proof(equality))],
            );
            complete.complete(root, root_bound)
        })
    })
}

fn transitive(
    left: &ScalarTerm,
    right: &ScalarTerm,
    left_proof: ProofNode,
    right_proof: ProofNode,
) -> ProofNode {
    ProofNode {
        conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_proof),
            middle_less_or_equal_right: Box::new(right_proof),
        },
    }
}

/// Distinct cited bounds `left <= middle` and `middle <= right` on a value
/// `middle`, in source order of the left leg and then of the right leg.
///
/// The right-leg index and the chains by outer endpoint are pure functions of
/// the scope, derived once per scope as citations.
struct TwoCitationChains<'a> {
    request: Request<'a>,
    /// Value-left cited bounds by their left endpoint.
    right_legs: Rc<ValueIndex<Citation>>,
}

impl<'a> TwoCitationChains<'a> {
    fn new(request: Request<'a>) -> Self {
        thread_local! {
            static RIGHT_LEGS: RefCell<ScopeDerived<ValueIndex<Citation>>> =
                const { RefCell::new(ScopeDerived::new()) };
        }
        let right_legs = scope_identity::derived(
            &RIGHT_LEGS,
            request.assumptions,
            request.semantic_axioms,
            || {
                let mut right_legs = ValueIndex::new();
                for (citation, _, left, _) in cited_bounds(request) {
                    if is_value(left) {
                        right_legs.push(left, citation);
                    }
                }
                right_legs
            },
        );
        Self {
            request,
            right_legs,
        }
    }

    fn find(
        &self,
        mut complete: impl FnMut(
            &'a ScalarTerm,
            &'a ScalarTerm,
            ProofNode,
            ProofNode,
        ) -> Option<ProofNode>,
    ) -> Option<ProofNode> {
        self.walk(|left_citation, left_fact, right_citation, right_fact| {
            chain_candidate(
                left_citation,
                left_fact,
                right_citation,
                right_fact,
                &mut complete,
            )
        })
    }

    /// Whether some chain names `endpoint` as an outer endpoint.
    fn touches(&self, endpoint: &ScalarTerm) -> bool {
        !self.by_outer_endpoint().candidates(endpoint).is_empty()
    }

    /// [`Self::find`] over only the chains that name `endpoint` as an outer
    /// endpoint, in the order `find` meets them.
    fn find_touching(
        &self,
        endpoint: &ScalarTerm,
        mut complete: impl FnMut(
            &'a ScalarTerm,
            &'a ScalarTerm,
            ProofNode,
            ProofNode,
        ) -> Option<ProofNode>,
    ) -> Option<ProofNode> {
        let Request {
            assumptions,
            semantic_axioms,
            ..
        } = self.request;
        self.by_outer_endpoint()
            .candidates(endpoint)
            .iter()
            .find_map(|&(left_citation, right_citation)| {
                chain_candidate(
                    left_citation,
                    left_citation.fact(assumptions, semantic_axioms),
                    right_citation,
                    right_citation.fact(assumptions, semantic_axioms),
                    &mut complete,
                )
            })
    }

    /// Visit every chain's `(left leg, right leg)` citations and facts, in walk
    /// order, until `visit` answers.
    fn walk<T>(
        &self,
        mut visit: impl FnMut(Citation, &'a Proposition, Citation, &'a Proposition) -> Option<T>,
    ) -> Option<T> {
        let Request {
            assumptions,
            semantic_axioms,
            ..
        } = self.request;
        cited_bounds(self.request)
            .filter(|(_, _, _, middle)| is_value(middle))
            .find_map(|(left_citation, left_fact, _, middle)| {
                self.right_legs
                    .candidates(middle)
                    .iter()
                    .find_map(|&right_citation| {
                        let right_fact = right_citation.fact(assumptions, semantic_axioms);
                        if std::ptr::eq(left_fact, right_fact) {
                            return None;
                        }
                        visit(left_citation, left_fact, right_citation, right_fact)
                    })
            })
    }

    /// Every chain [`Self::find`] walks, as `(left leg, right leg)` citations
    /// in walk order, listed under each distinct outer endpoint it names.
    fn by_outer_endpoint(&self) -> Rc<ValueIndex<(Citation, Citation)>> {
        thread_local! {
            static CHAINS: RefCell<ScopeDerived<ValueIndex<(Citation, Citation)>>> =
                const { RefCell::new(ScopeDerived::new()) };
        }
        scope_identity::derived(
            &CHAINS,
            self.request.assumptions,
            self.request.semantic_axioms,
            || {
                let mut index = ValueIndex::new();
                self.walk(|left_citation, left_fact, right_citation, right_fact| {
                    let (left, right) = outer_endpoints(left_fact, right_fact);
                    index.push(left, (left_citation, right_citation));
                    if right != left {
                        index.push(right, (left_citation, right_citation));
                    }
                    None::<()>
                });
                index
            },
        )
    }
}

/// The outer endpoints `left` and `right` of the chain `left <= middle <=
/// right` these two cited legs form.
fn outer_endpoints<'a>(
    left_fact: &'a Proposition,
    right_fact: &'a Proposition,
) -> (&'a ScalarTerm, &'a ScalarTerm) {
    let (Proposition::LessOrEqual(left, _), Proposition::LessOrEqual(_, right)) =
        (left_fact, right_fact)
    else {
        unreachable!("chain legs are cited order bounds")
    };
    (left, right)
}

fn chain_candidate<'a>(
    left_citation: Citation,
    left_fact: &'a Proposition,
    right_citation: Citation,
    right_fact: &'a Proposition,
    complete: &mut impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode, ProofNode) -> Option<ProofNode>,
) -> Option<ProofNode> {
    let (left, right) = outer_endpoints(left_fact, right_fact);
    complete(
        left,
        right,
        left_citation.proof(left_fact),
        right_citation.proof(right_fact),
    )
}
