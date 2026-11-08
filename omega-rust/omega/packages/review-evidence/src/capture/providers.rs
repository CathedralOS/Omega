//! Callable provider projection.
//!
//! Conformance classification is the entrance. Exact selected-plan joins and
//! external binding custody live beside it as independently discoverable
//! validation responsibilities.

pub(super) mod application_realizations;
mod boundary_policy;
pub(super) mod installation;
pub(crate) mod policy;
pub(crate) use boundary_policy::project_checked_boundary_application_policy;
pub(crate) use policy::project_checked_selected_provider_policy;
pub(super) mod intrinsics;
pub(super) mod selection;
