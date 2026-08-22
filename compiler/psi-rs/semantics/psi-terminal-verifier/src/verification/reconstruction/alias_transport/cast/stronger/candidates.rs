//! Verifier-local ordered stronger-alias fact candidates.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::distinct_same_carrier_values;

pub(super) fn any(
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
    mut complete: impl FnMut(&ScalarTerm, &ScalarTerm, usize) -> bool,
) -> bool {
    let facts = || requirements.iter().chain(semantic_axioms);
    facts()
        .filter_map(|equality| match equality {
            Proposition::Equal(left, right) => Some((left, right)),
            _ => None,
        })
        .any(|(equality_left, equality_right)| {
            [
                (equality_left, equality_right),
                (equality_right, equality_left),
            ]
            .into_iter()
            .filter(|(root, alias)| distinct_same_carrier_values(root, alias))
            .any(|(root, alias)| {
                facts()
                    .filter_map(|bound| match bound {
                        Proposition::LessOrEqual(left, right) => Some((left, right)),
                        _ => None,
                    })
                    .any(|(bound_left, bound_right)| {
                        let (retained_literal, endpoint) = if bound_left == alias {
                            (bound_right, 0)
                        } else if bound_right == alias {
                            (bound_left, 1)
                        } else {
                            return false;
                        };
                        let Some((integer_type, _)) = retained_literal.integer_value() else {
                            return false;
                        };
                        root.scalar_type() == psi_core::ScalarType::Integer(integer_type)
                            && complete(root, retained_literal, endpoint)
                    })
            })
        })
}
