//! Producer-local exact affine endpoint mapping for relaxation.

use psi_core::{IntegerValue, Proposition, ScalarTerm};
use psi_proof_kernel::CheckedIntegerAffineForm;

pub(super) fn mapped_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Option<Proposition> {
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
    let (bound_type, IntegerValue::Signed(bound)) = bound.integer_value()? else {
        return None;
    };
    if bound_type != form.integer_type() {
        return None;
    }
    let mapped = form
        .coefficient()
        .checked_mul(bound)?
        .checked_add(form.offset())?;
    let mapped = ScalarTerm::integer(form.integer_type(), IntegerValue::Signed(mapped)).ok()?;
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
