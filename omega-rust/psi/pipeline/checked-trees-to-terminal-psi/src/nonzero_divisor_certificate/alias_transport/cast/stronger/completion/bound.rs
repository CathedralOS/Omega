//! Fixed stronger alias root-bound construction for cast production.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::super::super::integer_evidence::closed_integer_relation;

pub(super) fn prove(
    root: &ScalarTerm,
    alias: &ScalarTerm,
    retained_literal: &ScalarTerm,
    source_endpoint: ScalarTerm,
    endpoint: usize,
    retained_bound: ProofNode,
    equality: ProofNode,
) -> Option<ProofNode> {
    let closed_bridge = if endpoint == 1 {
        closed_integer_relation(Proposition::LessOrEqual(
            source_endpoint.clone(),
            retained_literal.clone(),
        ))?
    } else {
        closed_integer_relation(Proposition::LessOrEqual(
            retained_literal.clone(),
            source_endpoint.clone(),
        ))?
    };
    let alias_bound = ProofNode {
        conclusion: if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint.clone(), alias.clone())
        } else {
            Proposition::LessOrEqual(alias.clone(), source_endpoint.clone())
        },
        rule: if endpoint == 1 {
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(closed_bridge),
                middle_less_or_equal_right: Box::new(retained_bound),
            }
        } else {
            ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(retained_bound),
                middle_less_or_equal_right: Box::new(closed_bridge),
            }
        },
    };
    Some(ProofNode {
        conclusion: if endpoint == 1 {
            Proposition::LessOrEqual(source_endpoint, root.clone())
        } else {
            Proposition::LessOrEqual(root.clone(), source_endpoint)
        },
        rule: ProofRule::IntegerLessOrEqualSubstitution {
            relation: Box::new(alias_bound),
            equality: Box::new(equality),
            endpoint,
        },
    })
}
