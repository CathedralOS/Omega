//! Independent replay of one closed endpoint relaxation after affine mapping.

use psi_core::Proposition;
use psi_proof_kernel::{CheckedIntegerAffineForm, check_integer_affine_bound_conversion};

use super::super::integer_evidence::closed_integer_less_or_equal;

mod mapping;

use mapping::mapped_bound;

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
