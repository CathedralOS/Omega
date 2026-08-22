//! Citation-preserving endpoint index for fixed alias transport.

use std::collections::BTreeMap;

use psi_core::{Proposition, ScalarTerm};

use super::super::integer_evidence::{Citation, cited_facts};

pub(super) fn indexed_bounds<'a>(
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

pub(super) fn distinct_same_carrier_values(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    left != right
        && matches!(left, ScalarTerm::Value { .. })
        && matches!(right, ScalarTerm::Value { .. })
        && left.scalar_type() == right.scalar_type()
}

pub(super) fn substitute_bound_endpoint(
    relation: &Proposition,
    replacement: &ScalarTerm,
    endpoint: usize,
) -> Proposition {
    let Proposition::LessOrEqual(left, right) = relation else {
        unreachable!("only order bounds are indexed")
    };
    if endpoint == 0 {
        Proposition::LessOrEqual(replacement.clone(), right.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), replacement.clone())
    }
}
