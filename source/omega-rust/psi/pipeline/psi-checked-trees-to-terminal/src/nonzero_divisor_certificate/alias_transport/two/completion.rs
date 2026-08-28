//! Fixed two-alias endpoint-substitution completion for production.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_admission::{ProofNode, ProofRule};

use super::super::index::substitute_bound_endpoint;

pub(super) fn prove(
    relation: &Proposition,
    root: &ScalarTerm,
    middle_alias: &ScalarTerm,
    endpoint: usize,
    relation_proof: ProofNode,
    inner_equality: ProofNode,
    outer_equality: ProofNode,
) -> ProofNode {
    let middle_bound = ProofNode {
        conclusion: substitute_bound_endpoint(relation, middle_alias, endpoint),
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(relation_proof),
            equality: Box::new(inner_equality),
            endpoint,
        },
    };
    ProofNode {
        conclusion: substitute_bound_endpoint(relation, root, endpoint),
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(middle_bound),
            equality: Box::new(outer_equality),
            endpoint,
        },
    }
}
