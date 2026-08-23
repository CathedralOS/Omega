//! Source-ordered direct landed-literal candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::equalities;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(&'a ScalarTerm, &'a ScalarTerm, ProofNode) -> Option<T>,
) -> Option<T> {
    equalities::exact_value_bindings(assumptions, semantic_axioms).find_map(
        |(citation, equality, root, literal)| complete(root, literal, citation.proof(equality)),
    )
}
