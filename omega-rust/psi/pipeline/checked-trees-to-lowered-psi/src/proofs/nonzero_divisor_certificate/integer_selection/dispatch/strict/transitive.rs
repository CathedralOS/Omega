//! Join a cited strict edge with an independently derived non-strict bound.

use super::super::super::super::integer_evidence::projected_facts;
use super::super::super::bound;
use super::{DefinitionIndex, ProofNode, ProofRule, Proposition, PropositionContext};

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let Proposition::LessThan(goal_left, goal_right) = goal else {
        return None;
    };
    for fact in projected_facts(assumptions, semantic_axioms) {
        let relations = match fact.proposition {
            Proposition::LessThan(_, _) => vec![fact.proof()],
            Proposition::LessOrEqual(left, right) => [
                super::adjacent(left, false)
                    .map(|previous| Proposition::LessThan(previous, right.clone())),
                super::adjacent(right, true).map(|next| Proposition::LessThan(left.clone(), next)),
            ]
            .into_iter()
            .flatten()
            .map(|conclusion| ProofNode {
                conclusion,
                rule: ProofRule::IntegerOrderDiscreteness {
                    relation: Box::new(fact.proof()),
                },
            })
            .collect(),
            _ => continue,
        };
        for relation in relations {
            let Proposition::LessThan(left, right) = &relation.conclusion else {
                continue;
            };
            // Rejoin the cited endpoint before searching the missing leg. The
            // bound selector has no recursive strict-order dispatch, and every
            // candidate has one independently cited strict premise.
            for strict_first in [false, true] {
                let (strict_goal, bound_goal) = if strict_first {
                    (
                        Proposition::LessThan(goal_left.clone(), right.clone()),
                        Proposition::LessOrEqual(right.clone(), goal_right.clone()),
                    )
                } else {
                    (
                        Proposition::LessThan(left.clone(), goal_right.clone()),
                        Proposition::LessOrEqual(goal_left.clone(), left.clone()),
                    )
                };
                let Some(strict) =
                    super::complete(&strict_goal, relation.clone(), assumptions, semantic_axioms)
                else {
                    continue;
                };
                let Some(nonstrict) = bound::prove(
                    context,
                    &bound_goal,
                    assumptions,
                    semantic_axioms,
                    definitions,
                ) else {
                    continue;
                };
                let (left_to_middle, middle_to_right) = if strict_first {
                    (strict, nonstrict)
                } else {
                    (nonstrict, strict)
                };
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerStrictOrderTransitivity {
                        left_to_middle: Box::new(left_to_middle),
                        middle_to_right: Box::new(middle_to_right),
                    },
                });
            }
        }
    }
    None
}
