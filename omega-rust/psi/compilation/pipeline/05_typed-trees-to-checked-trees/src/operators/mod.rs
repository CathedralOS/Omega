//! Checked boundary operator applications and candidate evidence.
//!
//! Shared declaration/selection queries belong to semantic validation.
//! This stage retains proof-backed applications and checked body candidates.

mod applications;
mod body_candidates;
mod provider_requirements;

pub(crate) use applications::bind_boundary_operator_application_demands;
pub(crate) use body_candidates::retain_boundary_body_candidates;
pub(crate) use provider_requirements::retain_provider_requirements;
