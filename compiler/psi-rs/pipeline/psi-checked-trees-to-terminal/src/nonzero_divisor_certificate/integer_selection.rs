//! Canonical fixed-integer proposition and bound selection.

use psi_core::{Proposition, PropositionContext};
use psi_proof_kernel::{PrimitiveJudgment, ProofNode, ProofRule};

use super::integer_evidence::{cited_facts, closed_integer_relation};
use super::{affine_selection, cast_selection};

pub(super) fn build(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    match goal {
        Proposition::Truth => Some(ProofNode {
            conclusion: Proposition::Truth,
            rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
        }),
        Proposition::LessOrEqual(_, _) => {
            prove_integer_bound(context, goal, assumptions, semantic_axioms)
        }
        Proposition::Conjunction(conjuncts) => Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::ConjunctionIntroduction(
                conjuncts
                    .iter()
                    .map(|conjunct| build(context, conjunct, assumptions, semantic_axioms))
                    .collect::<Option<Vec<_>>>()?,
            ),
        }),
        Proposition::Disjunction(disjuncts) => {
            let (index, disjunct) =
                disjuncts.iter().enumerate().find_map(|(index, disjunct)| {
                    build(context, disjunct, assumptions, semantic_axioms)
                        .map(|proof| (index, proof))
                })?;
            Some(ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::DisjunctionIntroduction {
                    disjunct: Box::new(disjunct),
                    index,
                },
            })
        }
        _ => None,
    }
}

fn prove_integer_bound(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };

    if let Some(proof) =
        prove_exact_or_closed_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

    if let Some(proof) = prove_two_fact_transitive_integer_bound(goal, assumptions, semantic_axioms)
    {
        return Some(proof);
    }

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
            if let Some(relation_proof) = prove_exact_or_closed_transitive_integer_bound(
                &relation,
                assumptions,
                semantic_axioms,
            )
            .or_else(|| {
                prove_two_fact_transitive_integer_bound(&relation, assumptions, semantic_axioms)
            })
            .or_else(|| affine_selection::prove(context, &relation, assumptions, semantic_axioms))
            {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation_proof),
                        equality: Box::new(citation.proof(fact)),
                        endpoint,
                    },
                });
            }
            if replacement.integer_value().is_none() {
                continue;
            }
            if let Some(relation) = closed_integer_relation(relation) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualSubstitution {
                        relation: Box::new(relation),
                        equality: Box::new(citation.proof(fact)),
                        endpoint,
                    },
                });
            }
        }
    }

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
    cast_selection::prove(context, goal, assumptions, semantic_axioms)
        .or_else(|| affine_selection::prove(context, goal, assumptions, semantic_axioms))
}

fn prove_two_fact_transitive_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (left_citation, left_fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(left, middle) = left_fact else {
            continue;
        };
        if left != goal_left {
            continue;
        }
        for (right_citation, right_fact) in cited_facts(assumptions, semantic_axioms) {
            let Proposition::LessOrEqual(right_middle, right) = right_fact else {
                continue;
            };
            if right_middle == middle && right == goal_right {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(left_citation.proof(left_fact)),
                        middle_less_or_equal_right: Box::new(right_citation.proof(right_fact)),
                    },
                });
            }
        }
    }
    None
}

fn prove_exact_or_closed_transitive_integer_bound(
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        if fact == goal {
            return Some(citation.proof(fact));
        }
    }
    for (citation, fact) in cited_facts(assumptions, semantic_axioms) {
        let Proposition::LessOrEqual(fact_left, fact_right) = fact else {
            continue;
        };
        if fact_left == goal_left {
            let tail = Proposition::LessOrEqual(fact_right.clone(), goal_right.clone());
            if let Some(tail) = closed_integer_relation(tail) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(citation.proof(fact)),
                        middle_less_or_equal_right: Box::new(tail),
                    },
                });
            }
        }
        if fact_right == goal_right {
            let head = Proposition::LessOrEqual(goal_left.clone(), fact_left.clone());
            if let Some(head) = closed_integer_relation(head) {
                return Some(ProofNode {
                    conclusion: goal.clone(),
                    rule: ProofRule::IntegerLessOrEqualTransitivity {
                        left_less_or_equal_middle: Box::new(head),
                        middle_less_or_equal_right: Box::new(citation.proof(fact)),
                    },
                });
            }
        }
    }
    None
}
