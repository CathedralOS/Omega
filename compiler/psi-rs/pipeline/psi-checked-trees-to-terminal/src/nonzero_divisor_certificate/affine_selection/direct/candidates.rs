//! Source-ordered direct retained-bound candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::Citation;
use super::super::{bounds, eligibility};

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a Proposition, Citation) -> Option<T>,
) -> Option<T> {
    for (citation, root_bound, root_left, root_right) in
        bounds::ordered(assumptions, semantic_axioms)
    {
        for root in eligibility::ordered_value_endpoints(root_left, root_right) {
            if let Some(result) = complete(root, root_bound, citation) {
                return Some(result);
            }
        }
    }
    None
}
