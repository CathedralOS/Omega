//! Producer-local exact affine endpoint mapping for relaxation.

use psi_core::Proposition;
use psi_proof_kernel::CheckedIntegerAffineForm;

mod endpoint;
mod orientation;
mod value;

pub(super) fn mapped_bound(
    form: &CheckedIntegerAffineForm,
    root_bound: &Proposition,
) -> Option<Proposition> {
    let endpoint = endpoint::select(form, root_bound)?;
    let mapped = value::mapped(form, endpoint.bound)?;
    Some(orientation::bound(form, mapped, endpoint.root_is_lower))
}
