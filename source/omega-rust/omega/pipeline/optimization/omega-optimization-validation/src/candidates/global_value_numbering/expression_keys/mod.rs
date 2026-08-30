//! Independent scalar-expression reconstruction group.
//!
//! The model defines three closed equivalence vocabularies. Obligation-free,
//! proof-certified, and compatible-policy reconstruction descend into separate
//! leaves; compatible-policy keeps its directional leader/redundant join.
//! Canonical commutative operand order is shared here and nowhere else.

use super::*;

mod compatible_policy;
mod model;
mod proof_certified;
mod total;

pub(crate) use compatible_policy::{
    independent_compatible_policy_scalar_leader, independent_compatible_policy_scalar_redundant,
};
pub(crate) use model::*;
pub(crate) use proof_certified::independent_proof_scalar_expression;
pub(crate) use total::independent_total_scalar_expression;

pub(crate) fn independent_pair(left: ValueId, right: ValueId) -> (ValueId, ValueId) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}
