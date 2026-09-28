//! Side-local endpoint and value utilities for fixed alias transport.

use semantic_vocabulary::{Proposition, ScalarTerm, ScalarType};

mod bounds;

pub(super) use bounds::indexed_bounds;

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
