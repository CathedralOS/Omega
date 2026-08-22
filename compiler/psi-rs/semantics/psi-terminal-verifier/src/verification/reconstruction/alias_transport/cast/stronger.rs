//! Closed-strengthened alias bounds for exact integer-cast reconstruction.

use psi_core::{Proposition, PropositionContext};

use super::super::distinct_same_carrier_values;

mod completion;

pub(super) fn retained(
    context: &PropositionContext,
    goal: &Proposition,
    requirements: &[Proposition],
    semantic_axioms: &[Proposition],
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
                            && completion::retained(
                                context,
                                goal,
                                semantic_axioms,
                                root,
                                retained_literal,
                                endpoint,
                            )
                    })
            })
        })
}
