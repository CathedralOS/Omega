//! Independent relaxed completion of one checked affine witness.

use psi_core::Proposition;
use psi_proof_kernel::CheckedIntegerAffineForm;

use super::super::relaxation;

pub(super) fn retained(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
    goal: &Proposition,
) -> bool {
    relaxation::retained(form, root_bound, goal)
}
