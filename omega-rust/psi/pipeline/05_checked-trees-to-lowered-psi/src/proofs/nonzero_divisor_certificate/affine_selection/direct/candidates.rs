//! Source-ordered direct retained-bound candidates for certificate production.

use proof_admission::ProofNode;
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::bounds;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, ProofNode) -> Option<T>,
) -> Option<T> {
    bounds::ordered(assumptions, semantic_axioms).find_map(
        |(citation, root_bound, root_left, root_right)| {
            bounds::value_endpoints(root_left, root_right)
                .find_map(|root| complete(root, citation.proof(root_bound)))
        },
    )
}
