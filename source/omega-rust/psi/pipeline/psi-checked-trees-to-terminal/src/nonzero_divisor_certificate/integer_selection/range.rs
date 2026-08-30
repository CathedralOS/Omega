//! Carrier-independent landed exact-operation range certificates.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    integer_affine_truth_bounds,
};

use super::super::integer_evidence::closed_integer_relation;

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    _assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let target = goal_target(goal)?;
    for mapped_proof in target_bounds(context, target, semantic_axioms) {
        if &mapped_proof.conclusion == goal {
            return Some(mapped_proof);
        }
        if let Some(proof) = relax(goal, mapped_proof) {
            return Some(proof);
        }
    }
    None
}

pub(super) fn target_bounds(
    context: &PropositionContext,
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Vec<ProofNode> {
    let Some((definition_axiom, root, divisor)) =
        semantic_axioms.iter().enumerate().rev().find_map(
            |(index, proposition)| match proposition {
                Proposition::Equal(
                    left,
                    ScalarTerm::ExactIntegerMultiply {
                        left: root,
                        right: divisor,
                        ..
                    }
                    | ScalarTerm::ExactIntegerDivide {
                        left: root,
                        right: divisor,
                        ..
                    }
                    | ScalarTerm::ExactIntegerRemainder {
                        left: root,
                        right: divisor,
                        ..
                    },
                ) if left == target => Some((index, root.as_ref(), divisor.as_ref())),
                Proposition::Equal(
                    ScalarTerm::ExactIntegerMultiply {
                        left: root,
                        right: divisor,
                        ..
                    }
                    | ScalarTerm::ExactIntegerDivide {
                        left: root,
                        right: divisor,
                        ..
                    }
                    | ScalarTerm::ExactIntegerRemainder {
                        left: root,
                        right: divisor,
                        ..
                    },
                    right,
                ) if right == target => Some((index, root.as_ref(), divisor.as_ref())),
                _ => None,
            },
        )
    else {
        return Vec::new();
    };
    let literal_axiom = if divisor.integer_value().is_some() {
        None
    } else {
        let Some(literal_axiom) = semantic_axioms[..definition_axiom]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, proposition)| match proposition {
                Proposition::Equal(left, right)
                    if left == divisor && right.integer_value().is_some() =>
                {
                    Some(index)
                }
                Proposition::Equal(left, right)
                    if right == divisor && left.integer_value().is_some() =>
                {
                    Some(index)
                }
                _ => None,
            })
        else {
            return Vec::new();
        };
        Some(literal_axiom)
    };
    let witness = IntegerAffineWitness {
        root: root.clone(),
        target: target.clone(),
        definition_axioms: vec![definition_axiom],
        literal_axioms: vec![literal_axiom],
    };
    let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
        return Vec::new();
    };
    let truth = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    let Ok(mapped_bounds) = integer_affine_truth_bounds(&form) else {
        return Vec::new();
    };
    mapped_bounds
        .into_iter()
        .map(|mapped| ProofNode {
            conclusion: mapped.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(truth.clone()),
                witness: witness.clone(),
            },
        })
        .collect()
}

fn goal_target(goal: &Proposition) -> Option<&ScalarTerm> {
    let Proposition::LessOrEqual(left, right) = goal else {
        return None;
    };
    match (left, right) {
        (target @ ScalarTerm::Value { .. }, literal) if literal.integer_value().is_some() => {
            Some(target)
        }
        (literal, target @ ScalarTerm::Value { .. }) if literal.integer_value().is_some() => {
            Some(target)
        }
        _ => None,
    }
}

fn relax(goal: &Proposition, mapped: ProofNode) -> Option<ProofNode> {
    let (
        Proposition::LessOrEqual(goal_left, goal_right),
        Proposition::LessOrEqual(mapped_left, mapped_right),
    ) = (goal, &mapped.conclusion)
    else {
        return None;
    };
    if goal_left == mapped_left {
        let tail = closed_integer_relation(Proposition::LessOrEqual(
            mapped_right.clone(),
            goal_right.clone(),
        ))?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(mapped),
                middle_less_or_equal_right: Box::new(tail),
            },
        });
    }
    if goal_right == mapped_right {
        let head = closed_integer_relation(Proposition::LessOrEqual(
            goal_left.clone(),
            mapped_left.clone(),
        ))?;
        return Some(ProofNode {
            conclusion: goal.clone(),
            rule: ProofRule::IntegerLessOrEqualTransitivity {
                left_less_or_equal_middle: Box::new(head),
                middle_less_or_equal_right: Box::new(mapped),
            },
        });
    }
    None
}
