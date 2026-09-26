//! One primitive access: projecting a placed field, the sealed request,
//! its specialization for stable, external and atomic lowering, and the
//! corresponded forms that carry device correspondence.

pub(crate) mod corresponded_atomic;
pub(crate) mod corresponded_external;
pub(crate) mod corresponded_stable;
pub(crate) mod corresponded_stable_compound;
pub(crate) mod field_projection;
pub(crate) mod primitive_request;
pub(crate) mod primitive_specialization;
