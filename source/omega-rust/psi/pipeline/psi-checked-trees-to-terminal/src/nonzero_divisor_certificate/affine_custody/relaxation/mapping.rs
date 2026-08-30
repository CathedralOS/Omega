//! Producer-local exact affine endpoint mapping for relaxation.

use psi_core::Proposition;
use psi_proof_admission::{CheckedIntegerAffineForm, map_integer_affine_bound};

pub(in super::super) fn mapped_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Option<Proposition> {
    map_integer_affine_bound(form, root_bound).ok()
}
