//! Retained literal recovery and closed-order replay for integer obligations.

use psi_core::{Proposition, ScalarTerm};

pub(super) fn retained_integer_term_values<'a>(
    term: &'a ScalarTerm,
    requirements: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
) -> impl Iterator<Item = (psi_core::IntegerType, psi_core::IntegerValue)> + 'a {
    std::iter::once(term.integer_value()).flatten().chain(
        requirements
            .iter()
            .chain(semantic_axioms)
            .filter_map(move |fact| {
                let Proposition::Equal(left, right) = fact else {
                    return None;
                };
                if left == term {
                    right.integer_value()
                } else if right == term {
                    left.integer_value()
                } else {
                    None
                }
            }),
    )
}

pub(super) fn closed_integer_less_or_equal(left: &ScalarTerm, right: &ScalarTerm) -> bool {
    let Some((left_type, left)) = left.integer_value() else {
        return false;
    };
    let Some((right_type, right)) = right.integer_value() else {
        return false;
    };
    left_type == right_type
        && left_type
            .compare(left, right)
            .is_some_and(|order| !order.is_gt())
}
