//! Fixed one-alias endpoint-substitution completion for production.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

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
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(relation_proof),
            equality: Box::new(equality),
            endpoint,
        },
    }
}
