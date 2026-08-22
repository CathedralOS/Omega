//! Verifier-local ordered fixed two-alias candidates.

use psi_core::{Proposition, ScalarTerm};

use super::super::index::{distinct_same_carrier_values, indexed_bounds};
use super::completion;

pub(super) fn any(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &Proposition) -> bool,
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    let bounds_by_endpoint = indexed_bounds(requirements, semantic_axioms);
    facts()
        .filter_map(|outer_equality| match outer_equality {
            Proposition::Equal(left, right) => Some((outer_equality, left, right)),
            _ => None,
        })
        .any(|(outer_equality, outer_left, outer_right)| {
            [(outer_left, outer_right), (outer_right, outer_left)]
                .into_iter()
                .filter(|(root, middle_alias)| distinct_same_carrier_values(root, middle_alias))
                .any(|(root, middle_alias)| {
                    facts()
                        .filter(|inner_equality| !std::ptr::eq(outer_equality, *inner_equality))
                        .filter_map(|inner_equality| match inner_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(inner_left, inner_right)| {
                            let bound_alias = if inner_left == middle_alias {
                                inner_right
                            } else if inner_right == middle_alias {
                                inner_left
                            } else {
                                return false;
                            };
                            if bound_alias == root
                                || !distinct_same_carrier_values(middle_alias, bound_alias)
                            {
                                return false;
                            }
                            bounds_by_endpoint.get(bound_alias).is_some_and(|bounds| {
                                bounds.iter().any(|(relation, endpoint)| {
                                    let root_bound =
                                        completion::retained(relation, root, *endpoint);
                                    complete(root, &root_bound)
                                })
                            })
                        })
                })
        })
}
