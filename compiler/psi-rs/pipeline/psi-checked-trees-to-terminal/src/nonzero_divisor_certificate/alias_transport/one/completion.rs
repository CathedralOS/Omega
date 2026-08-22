//! Fixed one-alias endpoint-substitution completion for production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::index::substitute_bound_endpoint;

pub(super) fn prove(
    relation: &Proposition,
    root: &ScalarTerm,
    endpoint: usize,
    relation_proof: ProofNode,
    equality: ProofNode,
) -> ProofNode {
    ProofNode {
        conclusion: substitute_bound_endpoint(relation, root, endpoint),
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(relation_proof),
            equality: Box::new(equality),
            endpoint,
        },
    }
}
