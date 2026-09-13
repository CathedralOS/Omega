//! Explicit conjunction projection and equality transport for integer bounds.

use proof_admission::{ProofNode, ProofRule};
use semantic_vocabulary::{Proposition, ScalarTerm};

use super::super::super::integer_evidence::{cited_facts, closed_integer_relation};
use super::super::exact;

pub(super) fn prove(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let mut pending = cited_facts(assumptions, semantic_axioms)
        .map(|(citation, fact)| citation.proof(fact))
        .collect::<Vec<_>>();
    while let Some(proof) = pending.pop() {
        match &proof.conclusion {
            Proposition::Conjunction(conjuncts) => {
                for (conjunct, conclusion) in conjuncts.iter().enumerate() {
                    pending.push(ProofNode {
                        conclusion: conclusion.clone(),
                        rule: ProofRule::ConjunctionElimination {
                            conjunction: Box::new(proof.clone()),
                            conjunct,
                        },
                    });
                }
            }
            Proposition::LessOrEqual(_, _) => {
                if let Some(proof) = complete(goal, proof, assumptions, semantic_axioms) {
                    return Some(proof);
                }
            }
            Proposition::LessThan(left, right) => {
                let weakened = ProofNode {
                    conclusion: Proposition::LessOrEqual(left.clone(), right.clone()),
                    rule: ProofRule::IntegerOrderWeakening {
                        relation: Box::new(proof),
                    },
                };
                if let Some(proof) = complete(goal, weakened, assumptions, semantic_axioms) {
                    return Some(proof);
                }
            }
            Proposition::Equal(left, right) => {
                // A closed endpoint is useful only after a separately cited
                // equality chain connects it to this obligation's subject.
                for literal in [left, right]
                    .into_iter()
                    .filter(|term| term.integer_value().is_some())
                {
                    for relation in [
                        Proposition::LessOrEqual(literal.clone(), goal_right.clone()),
                        Proposition::LessOrEqual(goal_left.clone(), literal.clone()),
                    ] {
                        if let Some(closed) = closed_integer_relation(relation)
                            && let Some(proof) =
                                complete(goal, closed, assumptions, semantic_axioms)
                        {
                            return Some(proof);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    None
}

fn complete(
    goal: &Proposition,
    mut relation: ProofNode,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (endpoint, target) in [goal_left, goal_right].into_iter().enumerate() {
        relation = replace_endpoint(relation, endpoint, target, assumptions, semantic_axioms)?;
    }
    (relation.conclusion == *goal).then_some(relation)
}

fn replace_endpoint(
    relation: ProofNode,
    endpoint: usize,
    target: &ScalarTerm,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(left, right) = &relation.conclusion else {
        return None;
    };
    let old = if endpoint == 0 { left } else { right };
    if old == target {
        return Some(relation);
    }
    let conclusion = if endpoint == 0 {
        Proposition::LessOrEqual(target.clone(), right.clone())
    } else {
        Proposition::LessOrEqual(left.clone(), target.clone())
    };
    if let Some(equality) = exact::prove(
        &Proposition::Equal(target.clone(), old.clone()),
        assumptions,
        semantic_axioms,
    ) {
        return Some(ProofNode {
            conclusion,
            rule: ProofRule::IntegerOrderSubstitution {
                relation: Box::new(relation),
                equality: Box::new(equality),
                endpoint,
            },
        });
    }
    let bridge = if endpoint == 0 {
        Proposition::LessOrEqual(target.clone(), old.clone())
    } else {
        Proposition::LessOrEqual(old.clone(), target.clone())
    };
    let bridge = closed_integer_relation(bridge)?;
    let (left, right) = if endpoint == 0 {
        (bridge, relation)
    } else {
        (relation, bridge)
    };
    Some(ProofNode {
        conclusion,
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left),
            middle_less_or_equal_right: Box::new(right),
        },
    })
}

#[cfg(test)]
mod tests;
