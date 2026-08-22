//! One closed endpoint relaxation after exact affine mapping.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{CheckedIntegerAffineForm, IntegerAffineWitness, ProofNode, ProofRule};

use super::super::integer_evidence::closed_integer_relation;

pub(super) fn prove(
    goal: &Proposition,
    form: &CheckedIntegerAffineForm,
    root_bound: &ProofNode,
    witness: IntegerAffineWitness,
) -> Option<ProofNode> {
    let mapped_bound = mapped_bound(form, &root_bound.conclusion)?;
    let affine = ProofNode {
        conclusion: mapped_bound,
        rule: ProofRule::IntegerAffineBound {
            root_bound: Box::new(root_bound.clone()),
            witness,
        },
    };
    relax_bound(goal, affine)
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
