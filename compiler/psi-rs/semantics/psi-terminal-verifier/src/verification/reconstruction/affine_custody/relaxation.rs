//! Independent replay of one closed endpoint relaxation after affine mapping.

use psi_core::{Proposition, ScalarTerm};
use psi_proof_kernel::{CheckedIntegerAffineForm, check_integer_affine_bound_conversion};

use super::super::integer_evidence::closed_integer_less_or_equal;

pub(super) fn retained(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    goal: &Proposition,
) -> bool {
    mapped_bound(form, root_bound).is_some_and(|mapped| {
        check_integer_affine_bound_conversion(form, root_bound, &mapped).is_ok()
            && closed_bound_relaxes_to_goal(&mapped, goal)
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
