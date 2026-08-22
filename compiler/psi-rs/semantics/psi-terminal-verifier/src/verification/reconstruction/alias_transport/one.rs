//! Independent exactly-one-alias retained order selection.

use psi_core::{Proposition, ScalarTerm};

use super::index::{distinct_same_carrier_values, indexed_bounds};

mod completion;

pub(super) fn retained(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    let bounds_by_endpoint = indexed_bounds(requirements, semantic_axioms);
    requirements
        .iter()
        .chain(semantic_axioms)
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(left, right)| {
            [(left, right), (right, left)]
                .into_iter()
                .filter(|(root, alias)| distinct_same_carrier_values(root, alias))
                .any(|(root, alias)| {
                    bounds_by_endpoint.get(alias).is_some_and(|bounds| {
                        bounds.iter().any(|(relation, endpoint)| {
                            let root_bound = completion::retained(relation, root, *endpoint);
                            complete(root, &root_bound)
                        })
                    })
                })
        })
}
