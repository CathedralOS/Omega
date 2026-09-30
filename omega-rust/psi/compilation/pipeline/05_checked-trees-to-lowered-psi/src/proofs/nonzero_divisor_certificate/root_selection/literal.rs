//! Root sources built from a cited equality that lands a value on an integer
//! literal, directly or through exactly one alias equality.
//!
//! Two families close the landed literal differently. The closed family
//! proposes both `literal <= root` and `root <= literal`, each closed at
//! `literal <= literal`; affine custody maps either one through its word. The
//! goal family closes the literal against the goal's own literal side
//! remapped into the root's carrier, which is exactly the root bound an exact
//! cast chain needs.

use std::cell::RefCell;

use proof_admission::{PrimitiveJudgment, ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

use super::super::cast_custody;
use super::super::integer_evidence::{
    Citation, cited_facts, closed_integer_relation, literal_value,
};
use super::super::scope_identity::{self, ScopeDerived};
use super::roster::{
    EndpointEqualities, ValueIndex, distinct_same_carrier_subjects,
    exact_value_binding_orientations, exact_value_bindings, ordered_at, oriented_equality,
    substitute_subject, value_aliases,
};
use super::{Complete, Request};

pub(super) fn closed_landed_literal(
    request: Request,
    complete: &mut Complete,
) -> Option<ProofNode> {
    exact_value_bindings(request).find_map(|(citation, equality, root, literal)| {
        let equality = citation.proof(equality);
        closed_literal_bounds(literal, [(root, &equality)])
            .into_iter()
            .find_map(|root_bound| complete(root, root_bound))
    })
}

/// `root == alias` and `alias == literal` from two distinct cited equalities.
pub(super) fn closed_landed_literal_through_alias(
    request: Request,
    complete: &mut Complete,
) -> Option<ProofNode> {
    // Exact value bindings keyed by their value, as `(citation, reversed)`
    // orientations: a pure function of the scope, derived once per scope.
    thread_local! {
        static LANDINGS: RefCell<ScopeDerived<ValueIndex<(Citation, bool)>>> =
            const { RefCell::new(ScopeDerived::new()) };
    }
    let landings = scope_identity::derived(
        &LANDINGS,
        request.assumptions,
        request.semantic_axioms,
        || {
            let mut landings = ValueIndex::new();
            for &(citation, reversed) in exact_value_binding_orientations(request).iter() {
                let (_, _, alias, _) = oriented_equality(request, citation, reversed);
                landings.push(alias, (citation, reversed));
            }
            landings
        },
    );
    value_aliases(request).find_map(|(outer_citation, outer_equality, root, alias)| {
        if root.scalar_type() != alias.scalar_type() {
            return None;
        }
        landings
            .candidates(alias)
            .iter()
            .find_map(|&(citation, reversed)| {
                let (inner_citation, inner_equality, _, literal) =
                    oriented_equality(request, citation, reversed);
                if std::ptr::eq(outer_equality, inner_equality) {
                    return None;
                }
                let outer = outer_citation.proof(outer_equality);
                let inner = inner_citation.proof(inner_equality);
                closed_literal_bounds(literal, [(alias, &inner), (root, &outer)])
                    .into_iter()
                    .find_map(|root_bound| complete(root, root_bound))
            })
    })
}

/// `literal <= subject` then `subject <= literal`, each closed at
/// `literal <= literal` and renamed through `renames`, innermost first.
fn closed_literal_bounds<const RENAMES: usize>(
    literal: &ScalarTerm,
    renames: [(&ScalarTerm, &ProofNode); RENAMES],
) -> [ProofNode; 2] {
    [1, 0].map(|endpoint| {
        let closed = ProofNode {
            conclusion: Proposition::LessOrEqual(literal.clone(), literal.clone()),
            rule: ProofRule::Primitive(PrimitiveJudgment::ClosedIntegerRelation),
        };
        substitute_subject(
            closed,
            literal,
            endpoint,
            renames.map(|(subject, equality)| (subject, equality.clone())),
        )
    })
}

pub(super) fn goal_landed_literal(request: Request, complete: &mut Complete) -> Option<ProofNode> {
    exact_value_bindings(request).find_map(|(citation, equality, root, literal)| {
        let equality = citation.proof(equality);
        [1, 0].into_iter().find_map(|endpoint| {
            let root_bound =
                goal_literal_bound(request.goal, root, literal, endpoint, [(root, &equality)])?;
            complete(root, root_bound)
        })
    })
}

/// `root == alias` and `alias == literal` from two distinct cited equalities
/// between integer subjects of one carrier.
pub(super) fn goal_landed_literal_through_alias(
    request: Request,
    complete: &mut Complete,
) -> Option<ProofNode> {
    let aliases_by_endpoint = EndpointEqualities::new(request);
    for (root_citation, root_equality) in cited_facts(request.assumptions, request.semantic_axioms)
    {
        let Proposition::Equal(root_left, root_right) = root_equality else {
            continue;
        };
        for (root, alias) in [(root_left, root_right), (root_right, root_left)] {
            if !distinct_same_carrier_subjects(root, alias) {
                continue;
            }
            for (literal_citation, literal_equality, literal) in
                aliases_by_endpoint.candidates(alias)
            {
                if std::ptr::eq(root_equality, literal_equality) {
                    continue;
                }
                let Some((integer_type, _)) = literal_value(literal) else {
                    continue;
                };
                if root.scalar_type() != ScalarType::Integer(integer_type) {
                    continue;
                }
                let root_equality_proof = root_citation.proof(root_equality);
                let literal_equality_proof = literal_citation.proof(literal_equality);
                let renames = [
                    (alias, &literal_equality_proof),
                    (root, &root_equality_proof),
                ];
                if let Some(proof) = [1, 0].into_iter().find_map(|endpoint| {
                    let root_bound =
                        goal_literal_bound(request.goal, root, literal, endpoint, renames)?;
                    complete(root, root_bound)
                }) {
                    return Some(proof);
                }
            }
        }
    }
    None
}

/// The goal's literal side remapped into `root`'s carrier, for a goal whose
/// value side sits at `endpoint`: `endpoint == 1` reads `literal <= target`.
pub(super) fn goal_literal_at(
    goal: &Proposition,
    root: &ScalarTerm,
    endpoint: usize,
) -> Option<ScalarTerm> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let ScalarType::Integer(root_type) = root.scalar_type() else {
        return None;
    };
    let (target, literal) = if endpoint == 1 {
        (goal_right, goal_left)
    } else {
        (goal_left, goal_right)
    };
    if !matches!(target, ScalarTerm::Value { .. }) {
        return None;
    }
    cast_custody::remap_integer_literal(literal, root_type)
}

/// Close `landed_literal` against the goal's remapped literal side and rename
/// it to `root` through `renames`, innermost first.
fn goal_literal_bound<const RENAMES: usize>(
    goal: &Proposition,
    root: &ScalarTerm,
    landed_literal: &ScalarTerm,
    endpoint: usize,
    renames: [(&ScalarTerm, &ProofNode); RENAMES],
) -> Option<ProofNode> {
    let goal_literal = goal_literal_at(goal, root, endpoint)?;
    let closed = closed_integer_relation(ordered_at(landed_literal, &goal_literal, endpoint))?;
    Some(substitute_subject(
        closed,
        &goal_literal,
        endpoint,
        renames.map(|(subject, equality)| (subject, equality.clone())),
    ))
}
