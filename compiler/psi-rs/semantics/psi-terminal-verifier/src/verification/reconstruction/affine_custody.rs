//! Independent affine-witness completion for obligation reconstruction.
//!
//! Evidence selection remains in the parent verifier. This module owns the
//! bounded witness frontier, exact mapped bound, and closed relaxation replay.

use psi_core::{Proposition, PropositionContext, ScalarTerm};
use psi_proof_kernel::{
    CheckedIntegerAffineForm, IntegerAffineWitness, check_integer_affine_bound_conversion,
    check_integer_affine_witness,
};

use super::closed_integer_less_or_equal;

pub(super) fn retained_from_root(
    context: &PropositionContext,
    goal: &Proposition,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
    root_bound: &Proposition,
) -> bool {
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    [goal_left, goal_right]
        .into_iter()
        .filter(|target| matches!(target, ScalarTerm::Value { .. }))
        .any(|target| {
            definition_words(context, semantic_axioms, root)
                .into_iter()
                .any(|definition_axioms| {
                    let witness = IntegerAffineWitness {
                        root: root.clone(),
                        target: target.clone(),
                        definition_axioms,
                    };
                    check_integer_affine_witness(context, semantic_axioms, &witness).is_ok_and(
                        |form| {
                            check_integer_affine_bound_conversion(&form, root_bound, goal).is_ok()
                                || mapped_bound(&form, root_bound).is_some_and(|mapped| {
                                    check_integer_affine_bound_conversion(
                                        &form, root_bound, &mapped,
                                    )
                                    .is_ok()
                                        && closed_bound_relaxes_to_goal(&mapped, goal)
                                })
                        },
                    )
                })
        })
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

fn closed_bound_relaxes_to_goal(mapped: &Proposition, goal: &Proposition) -> bool {
    let Proposition::LessOrEqual(mapped_left, mapped_right) = mapped else {
        return false;
    };
    let Proposition::LessOrEqual(goal_left, goal_right) = goal else {
        return false;
    };
    (goal_right == mapped_right && closed_integer_less_or_equal(goal_left, mapped_left))
        || (goal_left == mapped_left && closed_integer_less_or_equal(mapped_right, goal_right))
}

fn definition_words(
    context: &PropositionContext,
    semantic_axioms: &[Proposition],
    root: &ScalarTerm,
) -> Vec<Vec<usize>> {
    const MAX_DEFINITIONS: usize = 4;

    // This only prunes candidate words. Every retained prefix and final bound
    // is independently replayed by the proof-kernel checkers above.
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
