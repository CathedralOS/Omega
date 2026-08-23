//! Independent replay of one closed endpoint relaxation after affine mapping.

use psi_core::Proposition;
use psi_proof_kernel::{CheckedIntegerAffineForm, check_integer_affine_bound_conversion};

mod completion;
mod mapping;

pub(super) use mapping::mapped_bound;

pub(super) fn retained(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    goal: &Proposition,
) -> bool {
    mapped_bound(form, root_bound).is_some_and(|mapped| {
        check_integer_affine_bound_conversion(form, root_bound, &mapped).is_ok()
            && completion::retained(&mapped, goal)
    })
}
