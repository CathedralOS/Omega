//! Existing ordered-bound certificate selection across exact-shift words.

use psi_core::{IntegerSign, IntegerValue, Proposition, PropositionContext, ScalarTerm};
use psi_proof_admission::{
    IntegerAffineWitness, PrimitiveJudgment, ProofNode, ProofRule, check_integer_affine_witness,
    integer_affine_truth_bounds, map_integer_affine_bound,
};

use super::super::affine_custody::DefinitionIndex;
use super::super::integer_evidence::{cited_facts, closed_integer_relation};
use super::bound;

pub(super) fn prove_recursive(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    definitions: &mut DefinitionIndex,
) -> Option<ProofNode> {
    let target = goal_target(goal)?;
    let witness = shift_witness(target, semantic_axioms)?;
    let checked = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let (goal_bound, lower) = if goal_left == target {
        (goal_right, false)
    } else if goal_right == target {
        (goal_left, true)
    } else {
        return None;
    };
    let (integer_type, goal_value) = goal_bound.integer_value()?;
    if integer_type != checked.integer_type() {
        return None;
    }
    let goal_value = integer_as_i128(goal_value)?;
    let minimum = integer_as_i128(integer_type.minimum_value())?;
    let maximum = integer_as_i128(integer_type.maximum_value())?;
    let valid = |candidate: i128| {
        let candidate = scalar_from_i128(integer_type, candidate)?;
        let root_goal = if lower {
            Proposition::LessOrEqual(candidate, checked.root().clone())
        } else {
            Proposition::LessOrEqual(checked.root().clone(), candidate)
        };
        let mapped = map_integer_affine_bound(&checked, &root_goal).ok()?;
        let Proposition::LessOrEqual(left, right) = &mapped else {
            return None;
        };
        let mapped_bound = if left == target {
            right
        } else if right == target {
            left
        } else {
            return None;
        };
        let (_, mapped_value) = mapped_bound.integer_value()?;
        let mapped_value = integer_as_i128(mapped_value)?;
        Some(if lower {
            mapped_value >= goal_value
        } else {
            mapped_value <= goal_value
        })
    };
    let mut low = minimum;
    let mut high = maximum;
    if lower {
        while low < high {
            let middle = (low & high) + ((low ^ high) >> 1);
            if valid(middle)? {
                high = middle;
            } else {
                low = middle.checked_add(1)?;
            }
        }
    } else {
        while low < high {
            let difference = low ^ high;
            let middle = (low & high) + (difference >> 1) + (difference & 1);
            if valid(middle)? {
                low = middle;
            } else {
                high = middle.checked_sub(1)?;
            }
        }
    }
    if !valid(low)? {
        return None;
    }
    let candidate = scalar_from_i128(integer_type, low)?;
    let root_goal = if lower {
        Proposition::LessOrEqual(candidate, checked.root().clone())
    } else {
        Proposition::LessOrEqual(checked.root().clone(), candidate)
    };
    let root_proof = bound::prove(
        context,
        &root_goal,
        assumptions,
        semantic_axioms,
        definitions,
    )?;
    let mapped = map_integer_affine_bound(&checked, &root_goal).ok()?;
    let mapped_proof = ProofNode {
        conclusion: mapped.clone(),
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_proof),
            witness,
        },
    };
    if &mapped == goal {
        Some(mapped_proof)
    } else {
        relax(goal, mapped_proof)
    }
}

fn integer_as_i128(value: IntegerValue) -> Option<i128> {
    match value {
        IntegerValue::Signed(value) => Some(value),
        IntegerValue::Unsigned(value) => i128::try_from(value).ok(),
    }
}

fn scalar_from_i128(integer_type: psi_core::IntegerType, value: i128) -> Option<ScalarTerm> {
    let value = match integer_type.sign() {
        IntegerSign::Signed => IntegerValue::Signed(value),
        IntegerSign::Unsigned => IntegerValue::Unsigned(u128::try_from(value).ok()?),
    };
    ScalarTerm::integer(integer_type, value).ok()
}

pub(super) fn prove(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
) -> Option<ProofNode> {
    let target = goal_target(goal)?;
    let witness = shift_witness(target, semantic_axioms)?;
    let checked = check_integer_affine_witness(context, semantic_axioms, &witness).ok()?;
    for (citation, root_bound) in cited_facts(assumptions, semantic_axioms) {
        let Ok(mapped) = map_integer_affine_bound(&checked, root_bound) else {
            continue;
        };
        let mapped_proof = ProofNode {
            conclusion: mapped.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(citation.proof(root_bound)),
                witness: witness.clone(),
            },
        };
        if &mapped == goal {
            return Some(mapped_proof);
        }
        if let Some(proof) = relax(goal, mapped_proof) {
            return Some(proof);
        }
    }
    let truth = ProofNode {
        conclusion: Proposition::Truth,
        rule: ProofRule::Primitive(PrimitiveJudgment::Truth),
    };
    for mapped in integer_affine_truth_bounds(&checked).ok()? {
        let mapped_proof = ProofNode {
            conclusion: mapped.clone(),
            rule: ProofRule::IntegerAffineBound {
                root_bound: Box::new(truth.clone()),
                witness: witness.clone(),
            },
        };
        if &mapped == goal {
            return Some(mapped_proof);
        }
        if let Some(proof) = relax(goal, mapped_proof) {
            return Some(proof);
        }
    }
    None
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

fn shift_witness(
    target: &ScalarTerm,
    semantic_axioms: &[Proposition],
) -> Option<IntegerAffineWitness> {
    let mut current = target.clone();
    let mut before = semantic_axioms.len();
    let mut reverse_definitions = Vec::new();
    let mut reverse_literals = Vec::new();
    loop {
        let (index, expression) = semantic_axioms[..before]
            .iter()
            .enumerate()
            .rev()
            .find_map(|(index, proposition)| match proposition {
                Proposition::Equal(left, right) if left == &current => Some((index, right)),
                Proposition::Equal(left, right) if right == &current => Some((index, left)),
                _ => None,
            })?;
        let (operand, count) = match expression {
            ScalarTerm::ExactIntegerShiftLeft { value, count, .. }
            | ScalarTerm::ExactIntegerShiftRight { value, count, .. } => {
                (value.as_ref(), count.as_ref())
            }
            _ => return None,
        };
        let literal_axiom = if count.integer_value().is_some() {
            None
        } else {
            Some(semantic_axioms[..index].iter().enumerate().rev().find_map(
                |(literal_index, proposition)| match proposition {
                    Proposition::Equal(left, right)
                        if left == count && right.integer_value().is_some() =>
                    {
                        Some(literal_index)
                    }
                    Proposition::Equal(left, right)
                        if right == count && left.integer_value().is_some() =>
                    {
                        Some(literal_index)
                    }
                    _ => None,
                },
            )?)
        };
        reverse_definitions.push(index);
        reverse_literals.push(literal_axiom);
        current = operand.clone();
        before = index;
        if !semantic_axioms[..before]
            .iter()
            .any(|proposition| match proposition {
                Proposition::Equal(left, right) => {
                    (left == &current
                        && matches!(
                            right,
                            ScalarTerm::ExactIntegerShiftLeft { .. }
                                | ScalarTerm::ExactIntegerShiftRight { .. }
                        ))
                        || (right == &current
                            && matches!(
                                left,
                                ScalarTerm::ExactIntegerShiftLeft { .. }
                                    | ScalarTerm::ExactIntegerShiftRight { .. }
                            ))
                }
                _ => false,
            })
        {
            break;
        }
    }
    reverse_definitions.reverse();
    reverse_literals.reverse();
    Some(IntegerAffineWitness {
        root: current,
        target: target.clone(),
        definition_axioms: reverse_definitions,
        literal_axioms: reverse_literals,
    })
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
