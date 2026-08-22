//! Source-ordered direct retained-bound candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::bounds;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, ProofNode) -> Option<T>,
) -> Option<T> {
    for (citation, root_bound, root_left, root_right) in
        bounds::ordered(assumptions, semantic_axioms)
    {
        for root in bounds::value_endpoints(root_left, root_right) {
            if let Some(result) = complete(root, citation.proof(root_bound)) {
                return Some(result);
            }
        }
    }
    None
}
