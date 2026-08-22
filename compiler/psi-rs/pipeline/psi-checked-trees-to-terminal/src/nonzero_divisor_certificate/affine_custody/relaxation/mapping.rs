//! Producer-local exact affine endpoint mapping for relaxation.

use psi_core::{IntegerValue, Proposition, ScalarTerm};
use psi_proof_kernel::CheckedIntegerAffineForm;

mod endpoint;

pub(super) fn mapped_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Option<Proposition> {
    let endpoint = endpoint::select(form, root_bound)?;
    let bound = endpoint.bound;
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
        endpoint.root_is_lower
    } else {
        !endpoint.root_is_lower
    };
    Some(if target_is_left {
        Proposition::LessOrEqual(form.target().clone(), mapped)
    } else {
        Proposition::LessOrEqual(mapped, form.target().clone())
    })
}
