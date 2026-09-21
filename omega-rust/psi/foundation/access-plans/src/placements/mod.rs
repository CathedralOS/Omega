//! Placing a plan over a concrete range: the placement plan, admission and
//! authority, owned and borrowed resident custody, stable and atomic
//! resident views, and schema correspondence.

pub(crate) mod atomic_resident_views;
pub(crate) mod borrowed_view;
pub(crate) mod owned_atomic_resident_custody;
pub(crate) mod owned_external_correspondence;
pub(crate) mod owned_placement_lifecycle;
pub(crate) mod owned_resident_custody;
pub(crate) mod placement_admission;
pub(crate) mod placement_authority;
pub(crate) mod placement_plan;
pub(crate) mod resident_views;
pub(crate) mod schema_correspondence;
