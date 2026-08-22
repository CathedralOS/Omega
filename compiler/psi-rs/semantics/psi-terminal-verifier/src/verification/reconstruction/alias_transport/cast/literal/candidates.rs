//! Verifier-local ordered alias-landed-literal candidates.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::distinct_same_carrier_values;

pub(super) fn any(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &ScalarTerm) -> bool,
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    facts()
        .filter_map(|root_equality| match root_equality {
            Proposition::Equal(left, right) => Some((root_equality, left, right)),
            _ => None,
        })
        .any(|(root_equality, root_left, root_right)| {
            [(root_left, root_right), (root_right, root_left)]
                .into_iter()
                .filter(|(root, alias)| distinct_same_carrier_values(root, alias))
                .any(|(root, alias)| {
                    facts()
                        .filter(|literal_equality| !std::ptr::eq(root_equality, *literal_equality))
                        .filter_map(|literal_equality| match literal_equality {
                            Proposition::Equal(left, right) => Some((left, right)),
                            _ => None,
                        })
                        .any(|(literal_left, literal_right)| {
                            let literal = if literal_left == alias {
                                literal_right
                            } else if literal_right == alias {
                                literal_left
                            } else {
                                return false;
                            };
                            let Some((integer_type, _)) = literal.integer_value() else {
                                return false;
                            };
                            root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                                && complete(root, literal)
                        })
                })
        })
}
