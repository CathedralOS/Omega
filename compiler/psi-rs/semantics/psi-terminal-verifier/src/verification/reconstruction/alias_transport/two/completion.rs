//! Independent fixed two-alias endpoint-substitution replay.

use psi_core::{Proposition, ScalarTerm};

use super::super::index::substitute_bound_endpoint;

pub(super) fn retained(relation: &Proposition, root: &ScalarTerm, endpoint: usize) -> Proposition {
    substitute_bound_endpoint(relation, root, endpoint)
}
