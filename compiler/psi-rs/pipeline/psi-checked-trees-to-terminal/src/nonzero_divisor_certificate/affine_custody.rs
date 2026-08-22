//! Affine-witness completion for canonical order certificates.
//!
//! Evidence selection remains in the parent producer. This module owns the
//! bounded witness frontier, exact mapped bound, and optional closed relaxation
//! that complete one already-constructed affine root bound.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{
    CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, ProofRule, check_certificate,
    check_integer_affine_witness,
};

use super::integer_evidence::closed_integer_relation;

pub(super) fn prove_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    assumptions: &[Proposition],
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: ProofNode,
) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    for target in [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
    {
        for definition_axioms in definition_words(context, semantic_axioms, root) {
            let witness = IntegerAffineWitness {
                root: root.clone(),
                target: target.clone(),
                definition_axioms,
            };
            let direct = ProofNode {
                conclusion: goal.clone(),
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness: witness.clone(),
                },
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &direct).is_ok() {
                return Some(direct);
            }

            let Ok(form) = check_integer_affine_witness(context, semantic_axioms, &witness) else {
                continue;
            };
            let Some(mapped_bound) = mapped_bound(&form, &root_bound.conclusion) else {
                continue;
            };
            let affine = ProofNode {
                conclusion: mapped_bound,
                rule: ProofRule::IntegerAffineBound {
                    root_bound: Box::new(root_bound.clone()),
                    witness,
                },
            };
            let Some(relaxed) = relax_bound(goal, affine) else {
                continue;
            };
            if check_certificate(context, goal, assumptions, semantic_axioms, &relaxed).is_ok() {
                return Some(relaxed);
            }
        }
    }
    None
}

fn mapped_bound(form: &CheckedIntegerAffineForm, root_bound: &Proposition) -> Option<Proposition> {
    let Proposition::LessOrEqual(left, right) = root_bound else {
        return None;
    };
    let (bound, root_is_lower_endpoint) = if left == form.root() {
        (right, false)
    } else if right == form.root() {
        (left, true)
    } else {
        return None;
    };
    let (bound_type, psi_core::IntegerValue::Signed(bound)) = bound.integer_value()? else {
        return None;
    };
    if bound_type != form.integer_type() {
        return None;
    }
    let mapped = form
        .coefficient()
        .checked_mul(bound)?
        .checked_add(form.offset())?;
    let mapped =
        ScalarTerm::integer(form.integer_type(), psi_core::IntegerValue::Signed(mapped)).ok()?;
    let target_is_left = if form.coefficient() < 0 {
        root_is_lower_endpoint
    } else {
        !root_is_lower_endpoint
    };
    Some(if target_is_left {
        Proposition::LessOrEqual(form.target().clone(), mapped)
    } else {
        Proposition::LessOrEqual(mapped, form.target().clone())
    })
}

fn relax_bound(goal: &Proposition, affine: ProofNode) -> Option<ProofNode> {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return None;
    };
    let Proposition::LessOrEqual(affine_left, affine_right) = &affine.conclusion else {
        return None;
    };
    let (left_less_or_equal_middle, middle_less_or_equal_right) = if goal_right == affine_right {
        let bridge = closed_integer_relation(Proposition::LessOrEqual(
            goal_left.clone(),
            affine_left.clone(),
        ))?;
        (bridge, affine)
    } else if goal_left == affine_left {
        let bridge = closed_integer_relation(Proposition::LessOrEqual(
            affine_right.clone(),
            goal_right.clone(),
        ))?;
        (affine, bridge)
    } else {
        return None;
    };
    Some(ProofNode {
        conclusion: goal.clone(),
        rule: ProofRule::IntegerLessOrEqualTransitivity {
            left_less_or_equal_middle: Box::new(left_less_or_equal_middle),
            middle_less_or_equal_right: Box::new(middle_less_or_equal_right),
        },
    })
}

fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
) -> Vec<Vec<usize>> {
    const MAX_DEFINITIONS: usize = 4;

    // This is candidate pruning, not proof authority: only prefixes replayed
    // successfully by the kernel advance, and the completed proof is checked
    // again before it leaves this module.
    let mut words = Vec::new();
    let mut frontier = vec![(Vec::new(), 0)];
    for _ in 0..MAX_DEFINITIONS {
        let mut next = Vec::new();
        for (prefix, start) in frontier {
            for index in start..semantic_axioms.len() {
                let Proposition::Equal(left, right) = &semantic_axioms[index] else {
                    continue;
                };
                let mut word = prefix.clone();
                word.push(index);
                let continues = [left, right]
                    .into_iter()
                    .filter(|target| matches!(target, ScalarTerm::Value { .. }))
                    .any(|target| {
                        check_integer_affine_witness(
                            context,
                            semantic_axioms,
                            &IntegerAffineWitness {
                                root: root.clone(),
                                target: target.clone(),
                                definition_axioms: word.clone(),
                            },
                        )
                        .is_ok()
                    });
                if continues {
                    words.push(word.clone());
                    next.push((word, index + 1));
                }
            }
        }
        frontier = next;
    }
    words
}
