//! Producer-local sign-directed orientation of one mapped affine bound.

use semantic_vocabulary::{Proposition, ScalarTerm};
use proof_admission::CheckedIntegerAffineForm;

pub(super) fn bound(
    form: &CheckedIntegerAffineForm,
    mapped: ScalarTerm,
    root_is_lower: bool,
) -> Proposition {
    let target_is_left = if form.coefficient() < 0 {
        root_is_lower
    } else {
        !root_is_lower
    };
    if target_is_left {
        Proposition::LessOrEqual(form.target().clone(), mapped)
    } else {
        Proposition::LessOrEqual(mapped, form.target().clone())
    }
}
