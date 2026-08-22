//! Producer-local fixed two-equality affine endpoint selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{ProofNode, ProofRule};

use super::super::super::super::affine_selection;
use super::super::super::super::integer_evidence::cited_facts;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (outer_citation, outer_equality) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::Equal(outer_left, outer_right) = outer_equality else {
            continue;
        };
        for (old, middle_alias) in [(outer_left, outer_right), (outer_right, outer_left)] {
            let endpoint = if old == goal_left {
                0
            } else if old == goal_right {
                1
            } else {
                continue;
            };
            if !matches!(old, psi_core::ScalarTerm::Value { .. })
                || !matches!(middle_alias, psi_core::ScalarTerm::Value { .. })
                || old == middle_alias
                || old.scalar_type() != middle_alias.scalar_type()
            {
                continue;
            }
            for (inner_citation, inner_equality) in cited_facts(assumptions, semantic_axioms) {
                if std::ptr::eq(outer_equality, inner_equality) {
                    continue;
                }
                let Proposition::Equal(inner_left, inner_right) = inner_equality else {
                    continue;
                };
                let target_alias = if inner_left == middle_alias {
                    inner_right
                } else if inner_right == middle_alias {
                    inner_left
                } else {
                    continue;
                };
                if !matches!(target_alias, psi_core::ScalarTerm::Value { .. })
                    || target_alias == old
                    || target_alias == middle_alias
                    || target_alias.scalar_type() != old.scalar_type()
                {
                    continue;
                }
                let relation = if endpoint == 0 {
                    Proposition::LessOrEqual(target_alias.clone(), goal_right.clone())
                } else {
                    Proposition::LessOrEqual(goal_left.clone(), target_alias.clone())
                };
                let Some(affine) =
                    affine_selection::prove(context, &relation, assumptions, semantic_axioms)
                else {
                    continue;
                };
                let middle_relation = if endpoint == 0 {
                    Proposition::LessOrEqual(middle_alias.clone(), goal_right.clone())
                } else {
                    Proposition::LessOrEqual(goal_left.clone(), middle_alias.clone())
                };
                let inner = ProofNode {
                    conclusion: middle_relation,
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(affine),
                        equality: Box::new(inner_citation.proof(inner_equality)),
                        endpoint,
                    },
                };
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(inner),
                        equality: Box::new(outer_citation.proof(outer_equality)),
                        endpoint,
                    },
                });
            }
        }
    }
    None
}
