//! Direct landed-literal root-bound construction for cast production.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::integer_evidence::closed_integer_relation;

pub(super) fn prove(
    root: &ScalarTerm,
    landed_literal: &ScalarTerm,
    source_endpoint: ScalarTerm,
    endpoint: usize,
    equality: ProofNode,
) -> Option<ProofNode> {
    let closed_relation = if endpoint == 1 {
        Proposition::LessOrEqual(source_endpoint.clone(), landed_literal.clone())
    } else {
        Proposition::LessOrEqual(landed_literal.clone(), source_endpoint.clone())
    };
    let closed_relation = closed_integer_relation(closed_relation)?;
    Some(ProofNode {
        conclusion: if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint, root.clone())
        } else {
            Proposition::LessOrEqual(root.clone(), source_endpoint)
        },
        rule: ProofRule::IntegerOrderSubstitution {
            relation: Box::new(closed_relation),
            equality: Box::new(equality),
            endpoint,
        },
    })
}
