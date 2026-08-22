//! Source-ordered one-alias transitive candidates for certificate production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::ProofNode;

use super::super::super::equalities;
use super::super::TwoCitationChains;

pub(super) fn find<'a, T>(
    assumptions: &'a [Proposition],
    semantic_axioms: &'a [Proposition],
    mut complete: impl FnMut(
        &'a ScalarTerm,
        &'a ScalarTerm,
        &'a ScalarTerm,
        &'a ScalarTerm,
        ProofNode,
        ProofNode,
        ProofNode,
    ) -> Option<T>,
) -> Option<T> {
    let chains = TwoCitationChains::new(assumptions, semantic_axioms);
    for (equality_citation, equality, root, alias) in
        equalities::value_aliases(assumptions, semantic_axioms)
    {
        let result = chains.find(|left, right, left_proof, right_proof| {
            complete(
                root,
                alias,
                left,
                right,
                left_proof,
                right_proof,
                equality_citation.proof(equality),
            )
        });
        if result.is_some() {
            return result;
        }
    }
    None
}
