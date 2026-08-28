//! Independent direct completion of one checked affine witness.

use psi_core::Proposition;
use psi_proof_admission::{CheckedIntegerAffineForm, check_integer_affine_bound_conversion};

pub(super) fn retained(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    goal: &Proposition,
) -> bool {
    check_integer_affine_bound_conversion(form, root_bound, goal).is_ok()
}
