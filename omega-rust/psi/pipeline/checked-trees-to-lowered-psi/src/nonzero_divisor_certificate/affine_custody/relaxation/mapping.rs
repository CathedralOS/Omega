//! Producer-local exact affine endpoint mapping for relaxation.

use proof_admission::{CheckedIntegerAffineForm, map_integer_affine_bound};
use semantic_vocabulary::Proposition;

pub(in super::super) fn mapped_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Option<Proposition> {
    map_integer_affine_bound(form, root_bound).ok()
}
