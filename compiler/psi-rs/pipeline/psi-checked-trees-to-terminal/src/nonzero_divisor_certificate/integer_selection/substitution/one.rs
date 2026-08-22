//! Fixed one-equality integer-bound substitution production.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::integer_evidence::cited_facts;
use super::relation;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };

    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(equality_left, equality_right) = fact else {
            continue;
        };
        for (old, replacement) in [
            (equality_left, equality_right),
            (equality_right, equality_left),
        ] {
            let (endpoint, relation) = if old == goal_left {
                (
                    0,
                    Proposition::LessOrEqual(replacement.clone(), goal_right.clone()),
                )
            } else if old == goal_right {
                (
                    1,
                    Proposition::LessOrEqual(goal_left.clone(), replacement.clone()),
                )
            } else {
                continue;
            };
            if let Some(relation_proof) = relation::prove(
                context,
                &relation,
                replacement.integer_value().is_some(),
                assumptions,
                semantic_axioms,
            ) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation_proof),
                        equality: Box::new(citation.proof(fact)),
                        endpoint,
                    },
                });
            }
        }
    }

    None
}
