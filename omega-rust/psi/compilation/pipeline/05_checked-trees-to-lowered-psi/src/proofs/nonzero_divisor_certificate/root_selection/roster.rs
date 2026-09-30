//! Source-ordered views of the cited roster and the substitution nodes that
//! rename a bound's subject through cited equalities.
//!
//! Every view yields facts in citation order: assumptions first, then
//! semantic axioms, each in position order. Endpoint indexes keep that order
//! within each key, so a lookup never reorders candidates.
//!
//! The indexes and the exact-binding selection are pure functions of the
//! scope, and root sources consult them at many goals under one scope, so each
//! is derived once per scope as citations and resolved against the request's
//! slices on every lookup.

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::super::integer_evidence::{Citation, cited_facts};
use super::super::scope_identity::{self, ScopeDerived};
use super::Request;

/// One cited fact read as `(citation, fact, first side, second side)`.
pub(super) type CitedSides<'a> = (Citation, &'a Proposition, &'a ScalarTerm, &'a ScalarTerm);

pub(super) fn is_value(term: &ScalarTerm) -> bool {
    matches!(term, ScalarTerm::Value { .. })
}

/// Cited `left <= right` facts.
pub(super) fn cited_bounds<'a>(request: Request<'a>) -> impl Iterator<Item = CitedSides<'a>> + 'a {
    cited_facts(request.assumptions, request.semantic_axioms).filter_map(|(citation, fact)| {
        match fact {
            Proposition::LessOrEqual(left, right) => Some((citation, fact, left, right)),
            _ => None,
        }
    })
}

/// The value endpoints of one bound, left first.
pub(super) fn value_endpoints<'a>(
    left: &'a ScalarTerm,
    right: &'a ScalarTerm,
) -> impl Iterator<Item = &'a ScalarTerm> {
    [left, right]
        .into_iter()
        .filter(|endpoint| is_value(endpoint))
}

/// Cited `left == right` facts, each read `(left, right)` then `(right, left)`.
fn oriented_equalities<'a>(request: Request<'a>) -> impl Iterator<Item = CitedSides<'a>> + 'a {
    cited_facts(request.assumptions, request.semantic_axioms)
        .filter_map(|(citation, equality)| match equality {
            Proposition::Equal(left, right) => Some((citation, equality, left, right)),
            _ => None,
        })
        .flat_map(|(citation, equality, left, right)| {
            [
                (citation, equality, left, right),
                (citation, equality, right, left),
            ]
        })
}

/// The cited equality at `citation`, read right-first when `reversed`.
pub(super) fn oriented_equality<'a>(
    request: Request<'a>,
    citation: Citation,
    reversed: bool,
) -> CitedSides<'a> {
    let equality = citation.fact(request.assumptions, request.semantic_axioms);
    let Proposition::Equal(left, right) = equality else {
        unreachable!("oriented citations name cited equalities")
    };
    if reversed {
        (citation, equality, right, left)
    } else {
        (citation, equality, left, right)
    }
}

/// Oriented equalities `root == literal` that land a value on an integer
/// literal of the value's own carrier.
pub(super) fn exact_value_bindings<'a>(
    request: Request<'a>,
) -> impl Iterator<Item = CitedSides<'a>> + 'a {
    let bindings = exact_value_binding_orientations(request);
    (0..bindings.len()).map(move |index| {
        let (citation, reversed) = bindings[index];
        oriented_equality(request, citation, reversed)
    })
}

/// The `(citation, reversed)` orientations [`exact_value_bindings`] resolves.
pub(super) fn exact_value_binding_orientations(request: Request) -> Rc<Vec<(Citation, bool)>> {
    thread_local! {
        static BINDINGS: RefCell<ScopeDerived<Vec<(Citation, bool)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    scope_identity::derived(
        &BINDINGS,
        request.assumptions,
        request.semantic_axioms,
        || {
            cited_facts(request.assumptions, request.semantic_axioms)
                .filter_map(|(citation, equality)| match equality {
                    Proposition::Equal(left, right) => Some((citation, left, right)),
                    _ => None,
                })
                .flat_map(|(citation, left, right)| {
                    [
                        (citation, false, left, right),
                        (citation, true, right, left),
                    ]
                })
                .filter(|(_, _, root, literal)| {
                    is_value(root)
                        && literal.integer_value().is_some_and(|(integer_type, _)| {
                            root.scalar_type() == ScalarType::Integer(integer_type)
                        })
                })
                .map(|(citation, reversed, _, _)| (citation, reversed))
                .collect()
        },
    )
}

/// Oriented equalities `root == alias` between two distinct values.
pub(super) fn value_aliases<'a>(request: Request<'a>) -> impl Iterator<Item = CitedSides<'a>> + 'a {
    oriented_equalities(request)
        .filter(|(_, _, root, alias)| root != alias && is_value(root) && is_value(alias))
}

/// Value-keyed candidates in insertion order.
pub(super) struct ValueIndex<T> {
    by_value: BTreeMap<ScalarTerm, Vec<T>>,
}

impl<T> ValueIndex<T> {
    pub(super) fn new() -> Self {
        Self {
            by_value: BTreeMap::new(),
        }
    }

    pub(super) fn push(&mut self, value: &ScalarTerm, candidate: T) {
        self.by_value
            .entry(value.clone())
            .or_default()
            .push(candidate);
    }

    pub(super) fn candidates(&self, value: &ScalarTerm) -> &[T] {
        self.by_value.get(value).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Cited `LessOrEqual` facts keyed by either endpoint, each with the key's
/// endpoint position (0 for left, 1 for right), so a lookup by one side yields
/// candidate bounds without rescanning every fact.
pub(super) struct EndpointBounds<'a> {
    by_endpoint: Rc<ValueIndex<(Citation, usize)>>,
    request: Request<'a>,
}

impl<'a> EndpointBounds<'a> {
    pub(super) fn new(request: Request<'a>) -> Self {
        thread_local! {
            static INDEXES: RefCell<ScopeDerived<ValueIndex<(Citation, usize)>>> =
                const { RefCell::new(ScopeDerived::new()) };
        }
        let by_endpoint = scope_identity::derived(
            &INDEXES,
            request.assumptions,
            request.semantic_axioms,
            || {
                let mut index = ValueIndex::new();
                for (citation, _, left, right) in cited_bounds(request) {
                    index.push(left, (citation, 0));
                    if right != left {
                        index.push(right, (citation, 1));
                    }
                }
                index
            },
        );
        Self {
            by_endpoint,
            request,
        }
    }

    /// Each bound on `endpoint` as `(citation, fact, other endpoint,
    /// position of endpoint)`.
    pub(super) fn candidates(
        &self,
        endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, usize)> + '_ {
        self.by_endpoint
            .candidates(endpoint)
            .iter()
            .map(|&(citation, position)| {
                let fact = citation.fact(self.request.assumptions, self.request.semantic_axioms);
                let Proposition::LessOrEqual(left, right) = fact else {
                    unreachable!("the endpoint index holds only cited order bounds")
                };
                (
                    citation,
                    fact,
                    if position == 0 { right } else { left },
                    position,
                )
            })
    }

    /// [`Self::candidates`] when `endpoint` is a value, and nothing otherwise:
    /// the fixed alias walks carry only value-endpoint bounds.
    pub(super) fn value_candidates(
        &self,
        endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm, usize)> + '_ {
        let endpoint_is_value = is_value(endpoint);
        self.candidates(endpoint).filter(move |_| endpoint_is_value)
    }
}

/// Cited `Equal` facts keyed by either endpoint, so a lookup by one side yields
/// its aliases in source order without rescanning every fact.
pub(super) struct EndpointEqualities<'a> {
    /// Per endpoint, each incident equality and whether its other endpoint is
    /// the left side.
    by_endpoint: Rc<ValueIndex<(Citation, bool)>>,
    request: Request<'a>,
}

impl<'a> EndpointEqualities<'a> {
    pub(super) fn new(request: Request<'a>) -> Self {
        thread_local! {
            static INDEXES: RefCell<ScopeDerived<ValueIndex<(Citation, bool)>>> =
                const { RefCell::new(ScopeDerived::new()) };
        }
        let by_endpoint = scope_identity::derived(
            &INDEXES,
            request.assumptions,
            request.semantic_axioms,
            || {
                let mut index = ValueIndex::new();
                for (citation, fact) in cited_facts(request.assumptions, request.semantic_axioms) {
                    let Proposition::Equal(left, right) = fact else {
                        continue;
                    };
                    index.push(left, (citation, false));
                    if right != left {
                        index.push(right, (citation, true));
                    }
                }
                index
            },
        );
        Self {
            by_endpoint,
            request,
        }
    }

    /// Each equality on `endpoint` as `(citation, fact, other endpoint)`.
    pub(super) fn candidates(
        &self,
        endpoint: &ScalarTerm,
    ) -> impl Iterator<Item = (Citation, &'a Proposition, &'a ScalarTerm)> + '_ {
        self.by_endpoint
            .candidates(endpoint)
            .iter()
            .map(|&(citation, other_is_left)| {
                let fact = citation.fact(self.request.assumptions, self.request.semantic_axioms);
                let Proposition::Equal(left, right) = fact else {
                    unreachable!("the endpoint index holds only cited equalities")
                };
                (citation, fact, if other_is_left { left } else { right })
            })
    }
}

/// Two distinct integer subjects of one carrier that a cited equality may
/// bridge.
pub(super) fn distinct_same_carrier_subjects(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    // Two fresh reads can share a still-current field observation. Only cited
    // equalities authorize that bridge; an intervening write removes them during
    // reconstruction. Equal paths alone are never equality evidence.
    let is_integer_subject = |subject: &ScalarTerm| {
        matches!(
            subject,
            ScalarTerm::Value {
                scalar_type: ScalarType::Integer(_),
                ..
            } | ScalarTerm::IntegerField { .. }
        )
    };
    left != right
        && is_integer_subject(left)
        && is_integer_subject(right)
        && left.scalar_type() == right.scalar_type()
}

/// The order relation with `subject` at position `endpoint` and `bound` on the
/// other side: `bound <= subject` at endpoint 1, `subject <= bound` at 0.
pub(super) fn ordered_at(subject: &ScalarTerm, bound: &ScalarTerm, endpoint: usize) -> Proposition {
    if endpoint == 1 {
        Proposition::LessOrEqual(bound.clone(), subject.clone())
    } else {
        Proposition::LessOrEqual(subject.clone(), bound.clone())
    }
}

/// Rename the subject at `endpoint` of `relation` through each cited equality
/// in turn, innermost first; every step keeps `bound` on the other side.
pub(super) fn substitute_subject<'a>(
    relation: ProofNode,
    bound: &ScalarTerm,
    endpoint: usize,
    renames: impl IntoIterator<Item = (&'a ScalarTerm, ProofNode)>,
) -> ProofNode {
    renames
        .into_iter()
        .fold(relation, |relation, (subject, equality)| ProofNode {
            conclusion: ordered_at(subject, bound, endpoint),
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(relation),
                equality: Box::new(equality),
                endpoint,
            },
        })
}
