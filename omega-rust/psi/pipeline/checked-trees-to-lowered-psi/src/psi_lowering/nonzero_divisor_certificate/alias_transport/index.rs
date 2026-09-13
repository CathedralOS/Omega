//! Side-local endpoint and value utilities for fixed alias transport.

use semantic_vocabulary::{Proposition, ScalarTerm};

mod bounds;

pub(super) use bounds::indexed_bounds;

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
