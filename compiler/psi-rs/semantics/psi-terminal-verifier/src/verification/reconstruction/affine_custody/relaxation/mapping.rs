//! Independent exact affine endpoint mapping for relaxation replay.

use psi_core::Proposition;
use psi_proof_kernel::CheckedIntegerAffineForm;

mod endpoint;
mod value;

pub(super) fn mapped_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Option<Proposition> {
    let endpoint = endpoint::select(form, root_bound)?;
    let mapped = value::mapped(form, endpoint.bound)?;
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
